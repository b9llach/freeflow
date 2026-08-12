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
use crate::settings::{self, Settings};
use crate::stt::parakeet::ParakeetStt;
use crate::stt::whisper::WhisperStt;
use crate::stt::SttEngine;

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
    let prev_url = state.pipeline.settings.lock().ollama_base_url.clone();

    if let Some(k) = parse_key(&new_settings.hotkey) {
        state.hotkey_state.set_key(k);
    }
    state.hotkey_state.set_mode(new_settings.hotkey_mode);

    if prev_url != new_settings.ollama_base_url {
        let new_llm: Arc<dyn LlmProvider> =
            Arc::new(Ollama::new(new_settings.ollama_base_url.clone()));
        state.pipeline.set_llm(new_llm);
    }

    *state.pipeline.settings.lock() = new_settings.clone();
    settings::save(&app, &new_settings)?;
    Ok(())
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

    if !target.exists() {
        let url = format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file_name}?download=true"
        );
        stream_download(&app, &url, &target, stem).await?;
    }

    let pipeline = state.pipeline.clone();
    let target_clone = target.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let stt = WhisperStt::load(target_clone).map_err(|e| e.to_string())?;
        stt.warmup_blocking();
        pipeline.set_stt(Arc::new(stt) as Arc<dyn SttEngine>);
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

// ─── Parakeet ────────────────────────────────────────────────────────────

const PARAKEET_HF_REPO: &str = "csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8";
const PARAKEET_FILES: &[&str] = &[
    "encoder.int8.onnx",
    "decoder.int8.onnx",
    "joiner.int8.onnx",
    "tokens.txt",
];

#[tauri::command]
pub async fn download_parakeet_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String> {
    let model_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models")
        .join("parakeet-tdt-0.6b-v3-int8");
    std::fs::create_dir_all(&model_dir)?;

    for file in PARAKEET_FILES {
        let target = model_dir.join(file);
        if target.exists() {
            continue;
        }
        let url = format!(
            "https://huggingface.co/{PARAKEET_HF_REPO}/resolve/main/{file}?download=true"
        );
        stream_download(&app, &url, &target, file).await?;
    }

    let pipeline = state.pipeline.clone();
    let dir_clone = model_dir.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let stt = ParakeetStt::load(dir_clone).map_err(|e| e.to_string())?;
        stt.warmup_blocking();
        pipeline.set_stt(Arc::new(stt) as Arc<dyn SttEngine>);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    {
        let mut s = state.pipeline.settings.lock();
        s.parakeet_model_dir = Some(model_dir.clone());
        s.stt_backend = crate::settings::SttBackend::Parakeet;
        settings::save(&app, &s)?;
    }

    Ok(model_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn set_stt_backend(
    app: AppHandle,
    state: State<'_, AppState>,
    backend: crate::settings::SttBackend,
) -> Result<()> {
    use crate::settings::SttBackend;

    let pipeline = state.pipeline.clone();
    let (whisper_path, parakeet_dir) = {
        let s = state.pipeline.settings.lock();
        (s.whisper_model_path.clone(), s.parakeet_model_dir.clone())
    };

    tokio::task::spawn_blocking(move || -> Result<()> {
        match backend {
            SttBackend::Whisper => match whisper_path {
                Some(p) if p.exists() => {
                    let stt = WhisperStt::load(p).map_err(|e| e.to_string())?;
                    stt.warmup_blocking();
                    pipeline.set_stt(Arc::new(stt) as Arc<dyn SttEngine>);
                    Ok(())
                }
                _ => Err("no whisper model configured".to_string().into()),
            },
            SttBackend::Parakeet => match parakeet_dir {
                Some(d) if d.exists() => {
                    let stt = ParakeetStt::load(d).map_err(|e| e.to_string())?;
                    stt.warmup_blocking();
                    pipeline.set_stt(Arc::new(stt) as Arc<dyn SttEngine>);
                    Ok(())
                }
                _ => Err("no parakeet model downloaded".to_string().into()),
            },
        }
    })
    .await
    .map_err(|e| e.to_string())??;

    {
        let mut s = state.pipeline.settings.lock();
        s.stt_backend = backend;
        settings::save(&app, &s)?;
    }
    Ok(())
}
