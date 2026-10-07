pub mod http;
pub mod managed;
pub mod whisper;

use async_trait::async_trait;

#[async_trait]
pub trait SttEngine: Send + Sync {
    async fn transcribe(&self, samples: &[f32], sample_rate: u32, lang: &str) -> anyhow::Result<String>;
    fn name(&self) -> &str;
}

/// Placeholder engine used when no speech model is configured or loadable.
/// The pipeline recognises the "null" name and shows a toast instead of
/// attempting to transcribe.
pub struct NullStt;

#[async_trait]
impl SttEngine for NullStt {
    async fn transcribe(
        &self,
        _samples: &[f32],
        _sample_rate: u32,
        _lang: &str,
    ) -> anyhow::Result<String> {
        anyhow::bail!("no speech engine is loaded")
    }

    fn name(&self) -> &str {
        "null"
    }
}
