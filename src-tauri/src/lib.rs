pub mod audio;
pub mod commands;
pub mod db;
pub mod error;
pub mod hotkey;
pub mod indicator;
pub mod llm;
pub mod output;
pub mod pipeline;
pub mod settings;
pub mod stt;

use parking_lot::Mutex;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{async_runtime, Manager, WindowEvent};

use crate::commands::AppState;
use crate::db::Db;
use crate::hotkey::{parse_key, HotkeyEvent, HotkeyState};
use crate::llm::ollama::Ollama;
use crate::llm::LlmProvider;
use crate::output::{clipboard::ClipboardSink, paste::PasteSink, OutputSink};
use crate::pipeline::Pipeline;
use crate::stt::{parakeet::ParakeetStt, whisper::WhisperStt, SttEngine};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Force a full backtrace on every panic — we need this to diagnose
    // crashes on user machines where we don't have a console attached.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    install_panic_hook();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "freeflow2_lib=info,warn".into()),
        )
        .try_init()
        .ok();

    let mut builder = tauri::Builder::default();

    // Single-instance MUST be the first plugin registered. The callback
    // fires on the *original* running instance when a second launch is
    // attempted; that second process then exits immediately. We use it to
    // pull the existing main window forward so double-clicking the tray
    // icon or re-running the exe just re-focuses instead of spawning a
    // duplicate freeflow2.exe (which would double-install the hotkey
    // hook, double-grab the mic, etc).
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main(app);
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = settings::load(&handle);
            let settings = Arc::new(Mutex::new(settings));

            let db_path = handle
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("freeflow.sqlite");
            let db = Db::open(db_path)?;
            let db = Arc::new(Mutex::new(db));

            // Load the STT engine synchronously in setup so the pipeline has a
            // real transcriber the moment the hotkey listener starts. Warmup
            // still runs so the first hotkey press isn't paying the mmap
            // page-in cost.
            let stt: Arc<dyn SttEngine> = {
                use crate::settings::SttBackend;
                let (backend, whisper_path, parakeet_dir) = {
                    let s = settings.lock();
                    (
                        s.stt_backend,
                        s.whisper_model_path.clone(),
                        s.parakeet_model_dir.clone(),
                    )
                };
                match backend {
                    SttBackend::Whisper => match whisper_path {
                        Some(p) if p.exists() => match WhisperStt::load(p.clone()) {
                            Ok(stt) => {
                                tracing::info!("whisper model loaded, warming up");
                                stt.warmup_blocking();
                                tracing::info!("whisper warmup complete");
                                Arc::new(stt) as Arc<dyn SttEngine>
                            }
                            Err(e) => {
                                tracing::error!(error = ?e, "failed to load whisper model");
                                Arc::new(NullStt) as Arc<dyn SttEngine>
                            }
                        },
                        _ => {
                            tracing::warn!("whisper model not configured; transcription will fail until set in settings");
                            Arc::new(NullStt) as Arc<dyn SttEngine>
                        }
                    },
                    SttBackend::Parakeet => match parakeet_dir {
                        Some(d) if d.exists() => match ParakeetStt::load(d.clone()) {
                            Ok(stt) => {
                                tracing::info!("parakeet model loaded, warming up");
                                stt.warmup_blocking();
                                tracing::info!("parakeet warmup complete");
                                Arc::new(stt) as Arc<dyn SttEngine>
                            }
                            Err(e) => {
                                tracing::error!(error = ?e, "failed to load parakeet model");
                                Arc::new(NullStt) as Arc<dyn SttEngine>
                            }
                        },
                        _ => {
                            tracing::warn!("parakeet model not downloaded; transcription will fail until set in settings");
                            Arc::new(NullStt) as Arc<dyn SttEngine>
                        }
                    },
                }
            };

            let llm: Arc<dyn LlmProvider> =
                Arc::new(Ollama::new(settings.lock().ollama_base_url.clone()));

            let outputs: Vec<Arc<dyn OutputSink>> = vec![
                Arc::new(PasteSink::new(handle.clone(), settings.clone())),
                Arc::new(ClipboardSink::new(handle.clone())),
            ];

            let pipeline = Arc::new(Pipeline::new(
                handle.clone(),
                db.clone(),
                stt,
                llm,
                outputs,
                settings.clone(),
            ));

            let (hk_key, hk_mode) = {
                let s = settings.lock();
                (parse_key(&s.hotkey).unwrap_or(rdev::Key::ControlRight), s.hotkey_mode)
            };
            let hotkey_state = HotkeyState::new(hk_key, hk_mode);

            let pipeline_for_cb = pipeline.clone();
            hotkey::spawn_listener(hotkey_state.clone(), move |ev| match ev {
                HotkeyEvent::Start => {
                    if let Err(e) = pipeline_for_cb.start_recording() {
                        tracing::error!(error = ?e, "start_recording failed");
                    }
                }
                HotkeyEvent::Stop => {
                    let p = pipeline_for_cb.clone();
                    async_runtime::spawn(async move {
                        if let Err(e) = p.stop_and_process().await {
                            tracing::error!(error = ?e, "stop_and_process failed");
                        }
                    });
                }
            });

            app.manage(AppState {
                pipeline,
                hotkey_state,
            });

            let show_item = MenuItem::with_id(app, "show", "Open Freeflow", true, None::<&str>)?;
            let toggle_item =
                MenuItem::with_id(app, "toggle_hotkey", "Pause hotkey", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &toggle_item, &quit_item])?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .tooltip("Freeflow")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "toggle_hotkey" => {
                        if let Some(state) = app.try_state::<AppState>() {
                            static PAUSED: parking_lot::Mutex<bool> = parking_lot::Mutex::new(false);
                            let mut g = PAUSED.lock();
                            *g = !*g;
                            state.hotkey_state.set_enabled(!*g);
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::list_ollama_models,
            commands::start_recording,
            commands::stop_recording,
            commands::is_recording,
            commands::list_history,
            commands::delete_transcription,
            commands::clear_history,
            commands::capture_key_name,
            commands::set_hotkey_enabled,
            commands::pick_whisper_model,
            commands::download_whisper_model,
            commands::download_parakeet_model,
            commands::set_stt_backend,
            commands::get_platform,
            commands::list_downloaded_whisper_models,
            commands::list_input_devices,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Write panics to `<app_config_or_data_dir>/crash.log` in addition to the
/// default console output. Without this a crash on a user's machine is a
/// silent black box: the process disappears with no trace.
///
/// We wrap std::panic::set_hook rather than replacing it so the default
/// stderr output still fires when there's a console.
fn install_panic_hook() {
    // Resolve a stable location on Windows / macOS / Linux without needing
    // an AppHandle — the panic hook may fire before Tauri is up.
    let dir = dirs::data_dir()
        .or_else(dirs::config_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("com.freeflow.app");
    let _ = std::fs::create_dir_all(&dir);
    let log_path = dir.join("crash.log");

    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Timestamp + panic message + location + full backtrace.
        let ts = chrono::Utc::now().to_rfc3339();
        let backtrace = std::backtrace::Backtrace::force_capture();
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_string());

        let entry = format!(
            "\n\n===== PANIC {ts} =====\nversion: {version}\nlocation: {location}\nmessage: {payload}\n\nbacktrace:\n{backtrace}\n",
            version = env!("CARGO_PKG_VERSION"),
        );

        // Append (not overwrite) so we keep prior crash history.
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            use std::io::Write;
            let _ = f.write_all(entry.as_bytes());
            let _ = f.flush();
        }

        // Let the default hook run too so any attached console still prints.
        default(info);
    }));
}

fn show_main(app: &tauri::AppHandle) {
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = a.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
    });
}

struct NullStt;

#[async_trait::async_trait]
impl SttEngine for NullStt {
    async fn transcribe(
        &self,
        _samples: &[f32],
        _sample_rate: u32,
        _lang: &str,
    ) -> anyhow::Result<String> {
        anyhow::bail!("whisper model not configured — set whisper_model_path in settings")
    }
    fn name(&self) -> &str {
        "null"
    }
}
