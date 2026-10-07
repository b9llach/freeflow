use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use super::http::HttpStt;
use super::SttEngine;
use crate::phonon::{self, Runtime};

/// Phonon-2 run by Freeflow. A thin wrapper over `HttpStt` that makes sure the
/// private server is up (starting it if it died) before each transcription.
pub struct ManagedStt {
    runtime: Arc<Runtime>,
    inner: HttpStt,
}

impl ManagedStt {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        let inner = HttpStt::new(runtime.base_url(), phonon::MODEL);
        Self { runtime, inner }
    }
}

#[async_trait]
impl SttEngine for ManagedStt {
    async fn transcribe(&self, samples: &[f32], sample_rate: u32, lang: &str) -> Result<String> {
        self.runtime.ensure_running().await?;
        self.inner.transcribe(samples, sample_rate, lang).await
    }

    fn name(&self) -> &str {
        phonon::MODEL
    }
}
