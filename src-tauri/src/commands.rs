use futures_util::StreamExt;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db::Transcription;
use crate::error::Result;
use crate::hotkey::{key_to_name, parse_key, HotkeyState};
use crate::llm::ollama::Ollama;
use crate::llm::{LlmProvider, ModelInfo};
use crate::pipeline::Pipeline;
use crate::settings::{self, Settings, SttBackend};
use crate::stt::http::HttpStt;
use crate::stt::managed::ManagedStt;
use crate::stt::whisper::WhisperStt;
use crate::stt::{NullStt, SttEngine};

pub struct AppState {
    pub pipeline: Arc<Pipeline>,
    pub hotkey_state: HotkeyState,
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    Ok(state.pipeline.settings.lock().clone())
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    new_settings: Settings,
) -> Result<()> {
    let (prev_url, prev_backend, prev_stt_url, prev_stt_model) = {
        let s = state.pipeline.settings.lock();
        (
            s.ollama_base_url.clone(),
            s.stt_backend,
            s.stt_http_url.clone(),
            s.stt_http_model.clone(),
        )
    };

    if let Some(k) = parse_key(&new_settings.hotkey) {
        state.hotkey_state.set_key(k);
    }
    state.hotkey_state.set_mode(new_settings.hotkey_mode);

    if prev_url != new_settings.ollama_base_url {
        let new_llm: Arc<dyn LlmProvider> =
            Arc::new(Ollama::new(new_settings.ollama_base_url.clone()));
        state.pipeline.set_llm(new_llm);
    }

    // While the HTTP engine is active, edits to its URL or model take effect
    // immediately. Switching between engines goes through set_stt_backend.
    if new_settings.stt_backend == SttBackend::Http
        && prev_backend == SttBackend::Http
        && (prev_stt_url != new_settings.stt_http_url
            || prev_stt_model != new_settings.stt_http_model)
    {
        state.pipeline.set_stt(Arc::new(HttpStt::new(
            new_settings.stt_http_url.clone(),
            new_settings.stt_http_model.clone(),
        )));
    }

    *state.pipeline.settings.lock() = new_settings.clone();
    settings::save(&app, &new_settings)?;
    Ok(())
}

/// Switch the active speech engine and persist the choice. Whisper needs the
/// model loaded from disk (done off the async runtime); the HTTP engine is
/// just a client so it swaps in instantly. With Whisper selected but no
/// model on disk the pipeline gets a placeholder that tells the user what to
/// configure, instead of silently keeping the previous engine.
#[tauri::command]
pub async fn set_stt_backend(
    app: AppHandle,
    state: State<'_, AppState>,
    backend: SttBackend,
) -> Result<()> {
    let pipeline = state.pipeline.clone();
    let (whisper_path, http_url, http_model) = {
        let s = state.pipeline.settings.lock();
        (
            s.whisper_model_path.clone(),
            s.stt_http_url.clone(),
            s.stt_http_model.clone(),
        )
    };

    match backend {
        SttBackend::Http => {
            pipeline.set_stt(Arc::new(HttpStt::new(http_url, http_model)));
        }
        SttBackend::Phonon => {
            let rt = phonon_runtime(&app);
            if rt.is_installed() {
                let rt2 = rt.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = rt2.ensure_running().await {
                        tracing::error!(error = ?e, "could not start Phonon-2");
                    }
                });
                pipeline.set_stt(Arc::new(ManagedStt::new(rt)));
            } else {
                pipeline.set_stt(Arc::new(NullStt));
            }
        }
        SttBackend::Whisper => {
            tokio::task::spawn_blocking(move || -> Result<()> {
                match whisper_path {
                    Some(p) if p.exists() => {
                        let stt = WhisperStt::load(p).map_err(|e| e.to_string())?;
                        stt.warmup_blocking();
                        pipeline.set_stt(Arc::new(stt) as Arc<dyn SttEngine>);
                    }
                    _ => pipeline.set_stt(Arc::new(NullStt)),
                }
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())??;
        }
    }

    {
        let mut s = state.pipeline.settings.lock();
        s.stt_backend = backend;
        settings::save(&app, &s)?;
    }
    Ok(())
}

/// The managed Phonon-2 runtime, rooted in the app data folder.
pub fn phonon_runtime(app: &AppHandle) -> Arc<crate::phonon::Runtime> {
    // Local (non-roaming) app data: this is 1+ GB of machine-specific files,
    // and Roaming paths can contain redirected or cloud-synced reparse points
    // that uv refuses to build its Python links through (os error 448).
    let base = app
        .path()
        .app_local_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("phonon");
    crate::phonon::Runtime::init(base, crate::phonon::DEFAULT_PORT)
}

#[tauri::command]
pub async fn phonon_status(app: AppHandle) -> Result<crate::phonon::Status> {
    Ok(phonon_runtime(&app).status().await)
}

/// Install (or finish installing) Phonon-2, start it, make it the active
/// engine. Progress is streamed on `freeflow://phonon-setup`.
#[tauri::command]
pub async fn phonon_install(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let rt = phonon_runtime(&app);
    let emitter = app.clone();
    let progress: crate::phonon::ProgressFn = Arc::new(move |p| {
        let _ = emitter.emit("freeflow://phonon-setup", p);
    });
    rt.install(progress).await.map_err(|e| format!("{e:#}"))?;

    state.pipeline.set_stt(Arc::new(ManagedStt::new(rt)));
    let mut s = state.pipeline.settings.lock();
    s.stt_backend = SttBackend::Phonon;
    settings::save(&app, &s)?;
    Ok(())
}

/// Remove the private Python environment, model cache and server. Falls back
/// to Whisper (or the placeholder) so the app never points at a missing engine.
#[tauri::command]
pub async fn phonon_uninstall(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let was_active = state.pipeline.settings.lock().stt_backend == SttBackend::Phonon;
    if was_active {
        state.pipeline.set_stt(Arc::new(NullStt));
    }
    phonon_runtime(&app)
        .uninstall()
        .await
        .map_err(|e| format!("{e:#}"))?;
    if was_active {
        let mut s = state.pipeline.settings.lock();
        s.stt_backend = SttBackend::Whisper;
        settings::save(&app, &s)?;
    }
    Ok(())
}

/// Ping the speech server's `/models` route. Returns the served model id.
#[tauri::command]
pub async fn test_stt_server(url: String) -> Result<String> {
    crate::stt::http::probe(&url).await.map_err(Into::into)
}

#[tauri::command]
pub async fn list_ollama_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>> {
    let llm = state.pipeline.llm_arc();
    llm.list_models().await.map_err(Into::into)
}

#[tauri::command]
pub async fn start_recording(state: State<'_, AppState>) -> Result<()> {
    state.pipeline.start_recording().map_err(Into::into)
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> Result<Option<Transcription>> {
    state.pipeline.stop_and_process().await.map_err(Into::into)
}

#[tauri::command]
pub fn is_recording(state: State<'_, AppState>) -> bool {
    state.pipeline.is_recording()
}

#[tauri::command]
pub fn list_history(state: State<'_, AppState>, limit: Option<i64>) -> Result<Vec<Transcription>> {
    let db = state.pipeline.db.lock();
    db.list_recent(limit.unwrap_or(200))
}

#[tauri::command]
pub fn delete_transcription(state: State<'_, AppState>, id: String) -> Result<()> {
    let db = state.pipeline.db.lock();
    db.delete(&id)
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<()> {
    let db = state.pipeline.db.lock();
    db.clear_all()
}

#[tauri::command]
pub fn capture_key_name(key: String) -> String {
    parse_key(&key).map(key_to_name).unwrap_or(key)
}

#[tauri::command]
pub fn get_platform() -> &'static str {
    std::env::consts::OS
}

/// Enumerate every input device the audio host exposes, marking the one
/// cpal reports as the system default. Used by the settings UI to populate
/// the microphone picker.
#[tauri::command]
pub fn list_input_devices() -> Result<Vec<crate::audio::InputDeviceInfo>> {
    crate::audio::list_input_devices().map_err(Into::into)
}

/// Returns filenames of every `ggml-*.bin` currently sitting in the app's
/// models directory, so the UI can label already-downloaded options and
/// switch the primary button from "Download" to "Use downloaded".
#[tauri::command]
pub fn list_downloaded_whisper_models(app: AppHandle) -> Result<Vec<String>> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(it) => it,
        Err(_) => return Ok(vec![]),
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with("ggml-") && name.ends_with(".bin") {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

#[tauri::command]
pub fn set_hotkey_enabled(state: State<'_, AppState>, enabled: bool) -> Result<()> {
    state.hotkey_state.set_enabled(enabled);
    Ok(())
}

#[tauri::command]
pub async fn pick_whisper_model(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<()> {
    let p = PathBuf::from(&path);
    let pipeline = state.pipeline.clone();
    let p_clone = p.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let stt = WhisperStt::load(p_clone).map_err(|e| e.to_string())?;
        stt.warmup_blocking();
        pipeline.set_stt(Arc::new(stt) as Arc<dyn SttEngine>);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    let mut s = state.pipeline.settings.lock();
    s.whisper_model_path = Some(p);
    settings::save(&app, &s)?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum WhisperModelKind {
    TinyEn,
    BaseEn,
    SmallEn,
    MediumEn,
    Tiny,
    Base,
    Small,
    Medium,
    LargeV3,
}

impl WhisperModelKind {
    fn file_stem(self) -> &'static str {
        match self {
            Self::TinyEn => "tiny.en",
            Self::BaseEn => "base.en",
            Self::SmallEn => "small.en",
            Self::MediumEn => "medium.en",
            Self::Tiny => "tiny",
            Self::Base => "base",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::LargeV3 => "large-v3",
        }
    }
}

#[derive(serde::Serialize, Clone)]
struct DownloadProgress {
    name: String,
    downloaded: u64,
    total: Option<u64>,
}

/// Streams `url` into `target`, emitting `freeflow://download-progress`
/// events with `name` every ~512 KB. Uses a `.part` sidecar and atomic
/// rename so partial writes don't leave a corrupt "finished" file behind.
async fn stream_download(
    app: &AppHandle,
    url: &str,
    target: &std::path::Path,
    progress_name: &str,
) -> Result<()> {
    let resp = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let total = resp.content_length();

    let tmp = target.with_extension(format!(
        "{}.part",
        target.extension().and_then(|s| s.to_str()).unwrap_or("dl")
    ));
    let mut file = std::fs::File::create(&tmp)?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        if downloaded - last_emit >= 512 * 1024 {
            last_emit = downloaded;
            let _ = app.emit(
                "freeflow://download-progress",
                DownloadProgress {
                    name: progress_name.to_string(),
                    downloaded,
                    total,
                },
            );
        }
    }
    file.flush()?;
    drop(file);
    std::fs::rename(&tmp, target)?;
    let _ = app.emit(
        "freeflow://download-progress",
        DownloadProgress {
            name: progress_name.to_string(),
            downloaded,
            total,
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn download_whisper_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model: WhisperModelKind,
) -> Result<String> {
    let stem = model.file_stem();
    let file_name = format!("ggml-{stem}.bin");
    let models_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models");
    std::fs::create_dir_all(&models_dir)?;
    let target = models_dir.join(&file_name);

    crate::log_step(&format!(
        "whisper.download start model={} target={:?}",
        stem, target
    ));

    if !target.exists() {
        let url = format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file_name}?download=true"
        );
        stream_download(&app, &url, &target, stem).await?;
    }

    let final_size = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
    crate::log_step(&format!(
        "whisper.download complete size={} bytes",
        final_size
    ));

    let pipeline = state.pipeline.clone();
    let target_clone = target.clone();
    let stem_owned = stem.to_string();
    tokio::task::spawn_blocking(move || -> Result<()> {
        // Wrap the native-code entry points (WhisperStt::load, which mmaps
        // the model and constructs a whisper.cpp context, and warmup which
        // runs a full 500ms inference pass) in catch_unwind. A Rust panic
        // in either would otherwise unwind through spawn_blocking and, in
        // release mode, just terminate the worker without a diagnostic.
        // We still can't catch a native abort() / access violation this
        // way — that's what the SEH handler in lib.rs is for — but this
        // does at least surface library-level Rust panics with a proper
        // message.

        crate::log_step(&format!(
            "whisper.load start path={:?}", target_clone
        ));
        let load_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            WhisperStt::load(target_clone.clone())
        }));
        let stt = match load_result {
            Ok(Ok(stt)) => {
                crate::log_step("whisper.load ok");
                stt
            }
            Ok(Err(e)) => {
                let msg = format!("whisper.load returned Err: {e}");
                crate::log_step(&msg);
                return Err(msg.into());
            }
            Err(panic) => {
                let msg = format!("whisper.load PANICKED: {:?}", panic_msg(&panic));
                crate::log_step(&msg);
                return Err(msg.into());
            }
        };

        crate::log_step(&format!("whisper.warmup start model={}", stem_owned));
        let warmup_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            stt.warmup_blocking();
        }));
        match warmup_result {
            Ok(()) => crate::log_step("whisper.warmup ok"),
            Err(panic) => {
                // Warmup is an optimization, not a requirement. If it
                // panics, log it and continue — the model is still loaded
                // and real transcriptions will just pay the mmap page-in
                // cost on the first PTT press instead.
                crate::log_step(&format!(
                    "whisper.warmup PANICKED (continuing): {:?}",
                    panic_msg(&panic)
                ));
            }
        }

        pipeline.set_stt(Arc::new(stt) as Arc<dyn SttEngine>);
        crate::log_step("whisper.load pipeline set_stt done");
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    {
        let mut s = state.pipeline.settings.lock();
        s.whisper_model_path = Some(target.clone());
        settings::save(&app, &s)?;
    }

    Ok(target.to_string_lossy().to_string())
}

fn panic_msg(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

