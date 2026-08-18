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
    #[cfg(windows)]
    install_windows_seh_handler();
    write_startup_diagnostics();

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

/// Resolve `<local_app_data>/com.freeflow.app/crash.log` on every platform,
/// creating the directory if it doesn't exist. Used by every diagnostic
/// sink (Rust panic hook, Windows SEH handler, startup dump) so they all
/// funnel into the same file the user is asked to share.
fn crash_log_path() -> std::path::PathBuf {
    let dir = dirs::data_dir()
        .or_else(dirs::config_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("com.freeflow.app");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("crash.log")
}

/// Append a single timestamped line to the crash log. Used by
/// `commands.rs` to breadcrumb the exact step it's on during Whisper
/// load/warmup so a subsequent native crash shows what we were doing.
pub fn log_step(msg: &str) {
    let ts = chrono::Utc::now().to_rfc3339();
    let line = format!("[{ts}] {msg}\n");
    append_crash_log(&line);
}

/// Append `entry` to the crash log. Silently no-ops on I/O error since
/// there is nowhere else to report it from a panic / native exception
/// context.
fn append_crash_log(entry: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crash_log_path())
    {
        use std::io::Write;
        let _ = f.write_all(entry.as_bytes());
        let _ = f.flush();
    }
}

/// One-time startup dump so every crash.log we get back from a user has
/// the machine's baseline context at the top: app version, OS, CPU model,
/// and detected SIMD feature flags. This is what tells us whether a native
/// crash is a CPU-feature mismatch (whisper.cpp assuming AVX2 on a machine
/// that doesn't have it, etc.) or something else entirely.
fn write_startup_diagnostics() {
    let ts = chrono::Utc::now().to_rfc3339();
    let mut entry = format!(
        "\n===== STARTUP {ts} =====\n\
         version:  {version}\n\
         os:       {os} / {arch}\n",
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
    );

    #[cfg(target_arch = "x86_64")]
    {
        entry.push_str(&format!(
            "cpu:      sse2={} avx={} avx2={} fma={} avx512f={} f16c={} bmi1={} bmi2={}\n",
            is_x86_feature_detected!("sse2"),
            is_x86_feature_detected!("avx"),
            is_x86_feature_detected!("avx2"),
            is_x86_feature_detected!("fma"),
            is_x86_feature_detected!("avx512f"),
            is_x86_feature_detected!("f16c"),
            is_x86_feature_detected!("bmi1"),
            is_x86_feature_detected!("bmi2"),
        ));
    }

    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::*;
        unsafe {
            let mut info = SYSTEM_INFO::default();
            GetNativeSystemInfo(&mut info);
            entry.push_str(&format!(
                "cores:    {}\npage_sz:  {} bytes\n",
                info.dwNumberOfProcessors, info.dwPageSize,
            ));
        }
    }

    append_crash_log(&entry);
}

/// Native (SEH) exception handler for Windows. Catches things Rust's panic
/// system never sees — access violations, illegal instructions, stack
/// overflows, divide-by-zero, missing-DLL entry-point resolution failures —
/// and writes the exception code + faulting address to crash.log before
/// letting Windows terminate the process the usual way.
///
/// This is the diagnostic we need when whisper.cpp / onnxruntime dereference
/// a bad pointer or execute an instruction the CPU doesn't support: neither
/// of those trips Rust's panic hook, so without this the crash is silent.
#[cfg(windows)]
unsafe extern "system" fn windows_seh_handler(
    info: *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS,
) -> i32 {
    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

    if info.is_null() {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    let record = (*info).ExceptionRecord;
    if record.is_null() {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    let code = (*record).ExceptionCode.0 as u32;
    let addr = (*record).ExceptionAddress as usize;

    // Names of the exception codes we're most likely to see out of
    // whisper.cpp / onnxruntime / sherpa-onnx.
    let name = match code {
        0xC000_0005 => "EXCEPTION_ACCESS_VIOLATION",
        0xC000_001D => "EXCEPTION_ILLEGAL_INSTRUCTION",
        0xC000_0025 => "EXCEPTION_NONCONTINUABLE_EXCEPTION",
        0xC000_008C => "EXCEPTION_ARRAY_BOUNDS_EXCEEDED",
        0xC000_008E => "EXCEPTION_FLT_DIVIDE_BY_ZERO",
        0xC000_0094 => "EXCEPTION_INT_DIVIDE_BY_ZERO",
        0xC000_00FD => "EXCEPTION_STACK_OVERFLOW",
        0xC000_0135 => "STATUS_DLL_NOT_FOUND",
        0xC000_0139 => "STATUS_ENTRYPOINT_NOT_FOUND",
        0xC000_0409 => "STATUS_STACK_BUFFER_OVERRUN",
        0x8000_0003 => "EXCEPTION_BREAKPOINT",
        _ => "UNKNOWN",
    };

    // Extra info specific to access violations: parameter[0] is the
    // read/write flag (0=read, 1=write, 8=DEP), parameter[1] is the
    // faulting virtual address.
    let (rw, fault_addr) = if code == 0xC000_0005 && (*record).NumberParameters >= 2 {
        let params = &(*record).ExceptionInformation;
        let kind = match params[0] {
            0 => "read",
            1 => "write",
            8 => "DEP",
            _ => "unknown-op",
        };
        (kind, params[1] as usize)
    } else {
        ("n/a", 0usize)
    };

    let ts = chrono::Utc::now().to_rfc3339();
    let entry = format!(
        "\n===== NATIVE EXCEPTION {ts} =====\n\
         version:      {version}\n\
         code:         0x{code:08X} ({name})\n\
         address:      0x{addr:016X}\n\
         access:       {rw} @ 0x{fault_addr:016X}\n",
        version = env!("CARGO_PKG_VERSION"),
    );
    append_crash_log(&entry);

    // Continue Windows's default handling (terminate + WerFault) so the
    // process actually dies. Returning EXCEPTION_EXECUTE_HANDLER (1)
    // would swallow the exception and leave the process in an undefined
    // state, which is worse than letting it die cleanly.
    EXCEPTION_CONTINUE_SEARCH
}

#[cfg(windows)]
fn install_windows_seh_handler() {
    use windows::Win32::System::Diagnostics::Debug::SetUnhandledExceptionFilter;
    unsafe {
        SetUnhandledExceptionFilter(Some(windows_seh_handler));
    }
}

/// Write panics to `<app_config_or_data_dir>/crash.log` in addition to the
/// default console output. Without this a crash on a user's machine is a
/// silent black box: the process disappears with no trace.
///
/// We wrap std::panic::set_hook rather than replacing it so the default
/// stderr output still fires when there's a console.
fn install_panic_hook() {
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
            "\n===== PANIC {ts} =====\n\
             version:  {version}\n\
             location: {location}\n\
             message:  {payload}\n\
             \nbacktrace:\n{backtrace}\n",
            version = env!("CARGO_PKG_VERSION"),
        );
        append_crash_log(&entry);

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
