use async_trait::async_trait;
use parking_lot::Mutex;
use sherpa_rs::transducer::{TransducerConfig, TransducerRecognizer};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::SttEngine;

/// NVIDIA Parakeet TDT via sherpa-onnx. The model directory should contain
/// four files: encoder ONNX, decoder ONNX, joiner ONNX, and tokens.txt.
pub struct ParakeetStt {
    inner: Arc<Mutex<TransducerRecognizer>>,
    model_dir: PathBuf,
}

/// Locate the encoder/decoder/joiner/tokens files inside `dir`. sherpa-onnx
/// converted NeMo Parakeet models on Hugging Face use a mix of naming
/// conventions across releases (`encoder.onnx`, `encoder.int8.onnx`,
/// `encoder-epoch-...int8.onnx`), so we scan for the first file whose name
/// contains the role and ends with `.onnx`.
fn find_component(dir: &Path, role: &str) -> anyhow::Result<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("read model dir: {e}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|p| {
            let name = p
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            name.contains(role) && name.ends_with(".onnx")
        })
        .collect();

    // Prefer int8 variants (smaller, and what the community repos publish).
    candidates.sort_by_key(|p| {
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        (!name.contains("int8"), name)
    });

    candidates
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no {role} ONNX file found in {}", dir.display()))
}

fn find_tokens(dir: &Path) -> anyhow::Result<PathBuf> {
    let p = dir.join("tokens.txt");
    if p.exists() {
        return Ok(p);
    }
    Err(anyhow::anyhow!(
        "tokens.txt not found in {}",
        dir.display()
    ))
}

impl ParakeetStt {
    pub fn load(model_dir: PathBuf) -> anyhow::Result<Self> {
        let encoder = find_component(&model_dir, "encoder")?;
        let decoder = find_component(&model_dir, "decoder")?;
        let joiner = find_component(&model_dir, "joiner")?;
        let tokens = find_tokens(&model_dir)?;

        let path_str = |p: &Path| -> anyhow::Result<String> {
            Ok(p.to_str()
                .ok_or_else(|| anyhow::anyhow!("non-utf8 path"))?
                .to_string())
        };

        let config = TransducerConfig {
            encoder: path_str(&encoder)?,
            decoder: path_str(&decoder)?,
            joiner: path_str(&joiner)?,
            tokens: path_str(&tokens)?,
            // NeMo TDT models require model_type="nemo_transducer".
            model_type: "nemo_transducer".to_string(),
            num_threads: 2,
            sample_rate: 16_000,
            feature_dim: 80,
            decoding_method: "greedy_search".to_string(),
            provider: Some("cpu".to_string()),
            debug: false,
            ..Default::default()
        };

        let recognizer = TransducerRecognizer::new(config)
            .map_err(|e| anyhow::anyhow!("parakeet init: {e}"))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(recognizer)),
            model_dir,
        })
    }

    pub fn model_name(&self) -> String {
        self.model_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("parakeet")
            .to_string()
    }

    pub fn warmup_blocking(&self) {
        // 500 ms of silence at 16 kHz — pages the encoder/decoder/joiner
        // weights in and initializes internal buffers before the first
        // user-facing transcription.
        let silence = vec![0.0f32; 8_000];
        let mut guard = self.inner.lock();
        let _ = guard.transcribe(16_000, &silence);
    }
}

#[async_trait]
impl SttEngine for ParakeetStt {
    async fn transcribe(
        &self,
        samples: &[f32],
        sample_rate: u32,
        _lang: &str,
    ) -> anyhow::Result<String> {
        let recognizer = self.inner.clone();
        let samples = samples.to_vec();
        let text = tokio::task::spawn_blocking(move || {
            let mut guard = recognizer.lock();
            guard.transcribe(sample_rate, &samples)
        })
        .await?;
        Ok(text.trim().to_string())
    }

    fn name(&self) -> &str {
        "parakeet"
    }
}
