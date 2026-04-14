use async_trait::async_trait;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::OutputSink;

pub struct ClipboardSink {
    app: AppHandle,
}

impl ClipboardSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

#[async_trait]
impl OutputSink for ClipboardSink {
    async fn emit(&self, text: &str) -> anyhow::Result<()> {
        self.app
            .clipboard()
            .write_text(text.to_string())
            .map_err(|e| anyhow::anyhow!("clipboard: {e}"))?;
        Ok(())
    }

    fn name(&self) -> &str {
        "clipboard"
    }
}
