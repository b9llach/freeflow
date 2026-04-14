use async_trait::async_trait;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::SttEngine;

pub struct WhisperStt {
    ctx: Arc<Mutex<WhisperContext>>,
    model_path: PathBuf,
}

impl WhisperStt {
    pub fn load(model_path: PathBuf) -> anyhow::Result<Self> {
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or_else(|| anyhow::anyhow!("bad path"))?,
            WhisperContextParameters::default(),
        )
        .map_err(|e| anyhow::anyhow!("whisper load: {e}"))?;
        Ok(Self {
            ctx: Arc::new(Mutex::new(ctx)),
            model_path,
        })
    }

    pub fn model_name(&self) -> String {
        self.model_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("whisper")
            .to_string()
    }

    /// Run a short silent transcription to force the model's mmap'd pages
    /// into RAM and warm internal caches. Without this, the first real
    /// transcription pays the full page-in cost on the user's hotkey.
    pub fn warmup_blocking(&self) {
        let silence = vec![0.0f32; 8_000]; // 500ms @ 16kHz
        let guard = self.ctx.lock();
        let mut state = match guard.create_state() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "whisper warmup: create_state failed");
                return;
            }
        };
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_no_context(true);
        params.set_single_segment(true);
        params.set_suppress_blank(true);
        if let Err(e) = state.full(params, &silence) {
            tracing::warn!(error = %e, "whisper warmup: full failed");
        }
    }
}

#[async_trait]
impl SttEngine for WhisperStt {
    async fn transcribe(&self, samples: &[f32], _sample_rate: u32, lang: &str) -> anyhow::Result<String> {
        let ctx = self.ctx.clone();
        let samples = samples.to_vec();
        let lang = lang.to_string();
        tokio::task::spawn_blocking(move || {
            let guard = ctx.lock();
            let mut state = guard
                .create_state()
                .map_err(|e| anyhow::anyhow!("whisper state: {e}"))?;
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some(lang.as_str()));
            params.set_translate(false);
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);

            state
                .full(params, &samples)
                .map_err(|e| anyhow::anyhow!("whisper full: {e}"))?;

            let n = state
                .full_n_segments()
                .map_err(|e| anyhow::anyhow!("whisper segments: {e}"))?;
            let mut out = String::new();
            for i in 0..n {
                let seg = state
                    .full_get_segment_text(i)
                    .map_err(|e| anyhow::anyhow!("whisper get: {e}"))?;
                out.push_str(&seg);
            }
            Ok::<_, anyhow::Error>(out.trim().to_string())
        })
        .await?
    }

    fn name(&self) -> &str {
        "whisper"
    }
}
