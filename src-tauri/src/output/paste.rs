use async_trait::async_trait;
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::OutputSink;
use crate::settings::Settings;

pub struct PasteSink {
    app: AppHandle,
    settings: Arc<Mutex<Settings>>,
}

impl PasteSink {
    pub fn new(app: AppHandle, settings: Arc<Mutex<Settings>>) -> Self {
        Self { app, settings }
    }
}

#[cfg(target_os = "windows")]
const KEY_V: Key = Key::Other(0x56); // VK_V

// macOS: use the raw virtual keycode instead of Key::Unicode('v') because
// Unicode input forces enigo to call TSMGetInputSourceProperty to look up
// which physical key produces that character, and TSM asserts on the main
// thread on macOS 15+ — crashing the process from our tokio worker.
#[cfg(target_os = "macos")]
const KEY_V: Key = Key::Other(0x09); // kVK_ANSI_V

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
const KEY_V: Key = Key::Unicode('v');

fn send_paste(enigo: &mut Enigo) -> anyhow::Result<()> {
    // macOS: Cmd+V is the universal paste shortcut (Cmd+Shift+V is
    // "paste and match style" in many apps and is less reliable).
    // Windows / Linux: Ctrl+Shift+V is the plain-text paste in terminals,
    // Electron apps, browsers, and most modern software. Raw VK scancodes
    // instead of Unicode input so the modifier combo is interpreted as
    // a shortcut, not as text entry.

    #[cfg(target_os = "macos")]
    {
        enigo
            .key(Key::Meta, Direction::Press)
            .map_err(|e| anyhow::anyhow!("cmd press: {e}"))?;
        std::thread::sleep(Duration::from_millis(15));

        enigo
            .key(KEY_V, Direction::Press)
            .map_err(|e| anyhow::anyhow!("v press: {e}"))?;
        std::thread::sleep(Duration::from_millis(28));

        enigo
            .key(KEY_V, Direction::Release)
            .map_err(|e| anyhow::anyhow!("v release: {e}"))?;
        std::thread::sleep(Duration::from_millis(15));

        enigo
            .key(Key::Meta, Direction::Release)
            .map_err(|e| anyhow::anyhow!("cmd release: {e}"))?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| anyhow::anyhow!("ctrl press: {e}"))?;
        std::thread::sleep(Duration::from_millis(15));

        enigo
            .key(Key::Shift, Direction::Press)
            .map_err(|e| anyhow::anyhow!("shift press: {e}"))?;
        std::thread::sleep(Duration::from_millis(15));

        enigo
            .key(KEY_V, Direction::Press)
            .map_err(|e| anyhow::anyhow!("v press: {e}"))?;
        std::thread::sleep(Duration::from_millis(28));

        enigo
            .key(KEY_V, Direction::Release)
            .map_err(|e| anyhow::anyhow!("v release: {e}"))?;
        std::thread::sleep(Duration::from_millis(15));

        enigo
            .key(Key::Shift, Direction::Release)
            .map_err(|e| anyhow::anyhow!("shift release: {e}"))?;
        std::thread::sleep(Duration::from_millis(15));

        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| anyhow::anyhow!("ctrl release: {e}"))?;
    }

    Ok(())
}

#[async_trait]
impl OutputSink for PasteSink {
    async fn emit(&self, text: &str) -> anyhow::Result<()> {
        let app = self.app.clone();
        let text = text.to_string();
        let keep = self.settings.lock().copy_clipboard;

        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let prior = if !keep {
                app.clipboard().read_text().ok()
            } else {
                None
            };

            app.clipboard()
                .write_text(text.clone())
                .map_err(|e| anyhow::anyhow!("clipboard write: {e}"))?;

            // Let the OS clipboard manager propagate the change before we
            // fire the paste shortcut. Terminals and Electron apps poll on
            // focus, so this needs to be generous.
            std::thread::sleep(Duration::from_millis(90));

            let mut enigo = Enigo::new(&EnigoSettings::default())
                .map_err(|e| anyhow::anyhow!("enigo init: {e}"))?;

            send_paste(&mut enigo)?;

            // Only restore the prior clipboard if the user opted out of keeping
            // the transcript on the clipboard. Delay the restore long enough that
            // slow async paste readers still see the right payload.
            if let Some(prev) = prior {
                let app2 = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(450));
                    let _ = app2.clipboard().write_text(prev);
                });
            }

            Ok(())
        })
        .await??;
        Ok(())
    }

    fn name(&self) -> &str {
        "paste"
    }
}
