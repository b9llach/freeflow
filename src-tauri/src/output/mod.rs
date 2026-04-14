pub mod clipboard;
pub mod paste;

use async_trait::async_trait;

#[async_trait]
pub trait OutputSink: Send + Sync {
    async fn emit(&self, text: &str) -> anyhow::Result<()>;
    fn name(&self) -> &str;
}
