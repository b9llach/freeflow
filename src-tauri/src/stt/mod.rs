pub mod whisper;

use async_trait::async_trait;

#[async_trait]
pub trait SttEngine: Send + Sync {
    async fn transcribe(&self, samples: &[f32], sample_rate: u32, lang: &str) -> anyhow::Result<String>;
    fn name(&self) -> &str;
}
