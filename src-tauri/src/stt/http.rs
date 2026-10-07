use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::time::Duration;

use super::SttEngine;

/// Speech-to-text over an OpenAI-compatible `POST {base}/audio/transcriptions`
/// endpoint. This is what `fermion serve phonon-2` exposes (default
/// `http://127.0.0.1:8000/v1`), but any server that speaks the same route
/// works, local or remote.
///
/// The constructor never fails and never touches the network. URL problems
/// surface when a transcription is attempted, so typing into the settings
/// field does not produce an error for every keystroke.
pub struct HttpStt {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl HttpStt {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(120))
            .build()
            .expect("reqwest client");
        Self {
            base_url: base_url.into(),
            model: model.into(),
            client,
        }
    }
}

/// Accepts `127.0.0.1:8000`, `http://host:8000` or `http://host:8000/v1` and
/// returns a URL whose path is the API root. A bare host with no path gets
/// `/v1` appended because that is the OpenAI-compatible convention.
pub fn normalize_base_url(raw: &str) -> anyhow::Result<reqwest::Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        anyhow::bail!("speech server URL is empty");
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let mut url = reqwest::Url::parse(&with_scheme)
        .map_err(|e| anyhow::anyhow!("invalid speech server URL '{trimmed}': {e}"))?;
    if url.path().is_empty() || url.path() == "/" {
        url.set_path("/v1");
    }
    Ok(url)
}

fn endpoint(base: &reqwest::Url, tail: &str) -> reqwest::Url {
    let mut url = base.clone();
    let path = format!("{}/{}", base.path().trim_end_matches('/'), tail);
    url.set_path(&path);
    url
}

/// Encode mono f32 samples as a 16-bit PCM WAV file in memory.
pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn short(body: &str) -> String {
    let t = body.trim();
    if t.chars().count() > 300 {
        format!("{}...", t.chars().take(300).collect::<String>())
    } else {
        t.to_string()
    }
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

#[async_trait]
impl SttEngine for HttpStt {
    async fn transcribe(
        &self,
        samples: &[f32],
        sample_rate: u32,
        _lang: &str,
    ) -> anyhow::Result<String> {
        let base = normalize_base_url(&self.base_url)?;
        let url = endpoint(&base, "audio/transcriptions");

        let wav = encode_wav(samples, sample_rate);
        let part = Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")?;
        let form = Form::new()
            .part("file", part)
            .text("model", self.model.clone());

        let resp = self
            .client
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "could not reach speech server at {base}: {e}. \
                     Start it with: fermion serve {}",
                    self.model
                )
            })?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("failed reading speech server response: {e}"))?;
        if !status.is_success() {
            anyhow::bail!("speech server returned {status}: {}", short(&body));
        }

        let parsed: TranscriptionResponse = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "unexpected speech server response ({e}): {}",
                short(&body)
            )
        })?;
        Ok(parsed.text.trim().to_string())
    }

    fn name(&self) -> &str {
        &self.model
    }
}

/// Ping `{base}/models` and return the served model id (or "connected" if the
/// server answers but does not report one). Used by the settings "Test"
/// button so a stopped server is obvious before the first dictation.
pub async fn probe(base_url: &str) -> anyhow::Result<String> {
    let base = normalize_base_url(base_url)?;
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()?;
    let resp = client
        .get(endpoint(&base, "models"))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("could not reach speech server at {base}: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("speech server at {base} answered {status}");
    }
    let v: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    let id = v["data"][0]["id"]
        .as_str()
        .or_else(|| v["id"].as_str())
        .unwrap_or("connected");
    Ok(id.to_string())
}
