use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::audio::Recorder;
use crate::db::{Db, Transcription};
use crate::llm::{ChatMessage, LlmProvider};
use crate::output::OutputSink;
use crate::settings::Settings;
use crate::stt::SttEngine;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PipelineStatus {
    Idle,
    Recording,
    Thinking,
    Pasted,
}

pub struct Pipeline {
    pub app: AppHandle,
    pub db: Arc<Mutex<Db>>,
    pub stt: Mutex<Arc<dyn SttEngine>>,
    pub llm: Mutex<Arc<dyn LlmProvider>>,
    pub outputs: Vec<Arc<dyn OutputSink>>,
    pub settings: Arc<Mutex<Settings>>,
    recorder: Arc<Mutex<Option<Recorder>>>,
    recording: Arc<Mutex<bool>>,
}

impl Pipeline {
    pub fn new(
        app: AppHandle,
        db: Arc<Mutex<Db>>,
        stt: Arc<dyn SttEngine>,
        llm: Arc<dyn LlmProvider>,
        outputs: Vec<Arc<dyn OutputSink>>,
        settings: Arc<Mutex<Settings>>,
    ) -> Self {
        Self {
            app,
            db,
            stt: Mutex::new(stt),
            llm: Mutex::new(llm),
            outputs,
            settings,
            recorder: Arc::new(Mutex::new(None)),
            recording: Arc::new(Mutex::new(false)),
        }
    }

    pub fn set_stt(&self, stt: Arc<dyn SttEngine>) {
        *self.stt.lock() = stt;
    }

    pub fn stt_arc(&self) -> Arc<dyn SttEngine> {
        self.stt.lock().clone()
    }

    pub fn set_llm(&self, llm: Arc<dyn LlmProvider>) {
        *self.llm.lock() = llm;
    }

    pub fn llm_arc(&self) -> Arc<dyn LlmProvider> {
        self.llm.lock().clone()
    }

    pub fn is_recording(&self) -> bool {
        *self.recording.lock()
    }

    fn set_status(&self, status: PipelineStatus) {
        let _ = self.app.emit("freeflow://status", status);
    }

    fn finish_idle(&self) {
        crate::indicator::hide(&self.app);
        self.set_status(PipelineStatus::Idle);
    }

    pub fn start_recording(&self) -> anyhow::Result<()> {
        let mut rec = self.recorder.lock();
        if rec.is_some() {
            return Ok(());
        }
        *rec = Some(Recorder::start()?);
        *self.recording.lock() = true;
        if self.settings.lock().show_indicator {
            crate::indicator::show(&self.app);
        }
        self.set_status(PipelineStatus::Recording);
        Ok(())
    }

    pub async fn stop_and_process(&self) -> anyhow::Result<Option<Transcription>> {
        let rec = { self.recorder.lock().take() };
        *self.recording.lock() = false;

        let Some(rec) = rec else {
            self.finish_idle();
            return Ok(None);
        };
        let samples = rec.stop_and_take();

        if samples.len() < 16_000 / 4 {
            let _ = self
                .app
                .emit("freeflow://toast", "Recording too short, ignored");
            self.finish_idle();
            return Ok(None);
        }

        self.set_status(PipelineStatus::Thinking);

        let started = std::time::Instant::now();
        let lang = self.settings.lock().whisper_language.clone();
        let stt = self.stt_arc();
        let raw = stt
            .transcribe(&samples, crate::audio::TARGET_SAMPLE_RATE, &lang)
            .await?;

        if raw.trim().is_empty() {
            let _ = self
                .app
                .emit("freeflow://toast", "No speech detected");
            self.finish_idle();
            return Ok(None);
        }

        let (llm_enabled, system_prompt_base, llm_model, vocabulary) = {
            let s = self.settings.lock();
            (
                s.llm_enabled,
                s.system_prompt.clone(),
                s.ollama_model.clone(),
                s.vocabulary.clone(),
            )
        };

        let system_prompt = build_system_prompt(&system_prompt_base, &vocabulary);

        let cleaned = if llm_enabled {
            let msgs = vec![
                ChatMessage {
                    role: "system".into(),
                    content: system_prompt.clone(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: raw.clone(),
                },
            ];
            let llm = self.llm_arc();
            match llm.chat(&llm_model, &msgs).await {
                Ok(c) => Some(strip_wrappers(&c)),
                Err(e) => {
                    tracing::warn!(error = ?e, "llm cleanup failed, using raw");
                    let _ = self
                        .app
                        .emit("freeflow://toast", format!("LLM cleanup failed: {e}"));
                    None
                }
            }
        } else {
            None
        };

        let final_text = cleaned.clone().unwrap_or_else(|| raw.clone());

        let tx = Transcription {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now().to_rfc3339(),
            raw_text: raw.clone(),
            cleaned_text: cleaned.clone(),
            audio_path: None,
            stt_model: Some(stt.name().to_string()),
            llm_model: if llm_enabled { Some(llm_model.clone()) } else { None },
            duration_ms: Some(started.elapsed().as_millis() as i64),
            meta: None,
        };

        {
            let db = self.db.lock();
            db.insert_transcription(&tx)?;
            db.insert_message(&tx.id, "system", &system_prompt)?;
            db.insert_message(&tx.id, "user", &raw)?;
            if let Some(c) = &cleaned {
                db.insert_message(&tx.id, "assistant", c)?;
            }
        }

        let (auto_paste, copy_clipboard) = {
            let s = self.settings.lock();
            (s.auto_paste, s.copy_clipboard)
        };

        let mut paste_succeeded = false;
        for sink in &self.outputs {
            let should = match sink.name() {
                "paste" => auto_paste,
                "clipboard" => copy_clipboard,
                _ => true,
            };
            if !should {
                continue;
            }
            match sink.emit(&final_text).await {
                Ok(_) if sink.name() == "paste" => paste_succeeded = true,
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = ?e, sink = sink.name(), "output emit failed");
                }
            }
        }

        let _ = self.app.emit("freeflow://transcription", &tx);

        if paste_succeeded {
            self.set_status(PipelineStatus::Pasted);
            let app = self.app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(1500)).await;
                crate::indicator::hide(&app);
                let _ = app.emit("freeflow://status", PipelineStatus::Idle);
            });
        } else {
            self.finish_idle();
        }

        Ok(Some(tx))
    }
}

fn build_system_prompt(base: &str, vocabulary: &[String]) -> String {
    let mut out = base.trim().to_string();
    let entries: Vec<&str> = vocabulary
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if !entries.is_empty() {
        out.push_str(
            "\n\nThe user has provided this vocabulary and these facts. \
             Prefer these spellings and terms when the speech transcription \
             contains an obvious mishearing of any of them:\n",
        );
        for v in entries {
            out.push_str("- ");
            out.push_str(v);
            out.push('\n');
        }
    }
    out
}

fn strip_wrappers(s: &str) -> String {
    let t = s.trim();
    let t = t.trim_matches('"').trim_matches('\'');
    if let Some(rest) = t.strip_prefix("```") {
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim().trim_start_matches("text").trim().to_string();
        }
    }
    t.to_string()
}
