use serde::{Deserialize, Deserializer, Serialize};
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
// Container-level default so ANY missing / renamed / added field falls back
// to Settings::default() individually — instead of the whole file failing to
// parse and dropping the user to full defaults. This is what lets old
// settings.json files survive schema changes across app versions.
#[serde(default)]
pub struct Settings {
    pub ollama_base_url: String,
    pub ollama_model: String,
    pub system_prompt: String,
    pub hotkey: String,
    pub hotkey_mode: HotkeyMode,
    pub auto_paste: bool,
    pub copy_clipboard: bool,
    pub whisper_model_path: Option<PathBuf>,
    pub whisper_language: String,
    pub show_indicator: bool,
    pub llm_enabled: bool,
    pub vocabulary: Vec<String>,
    pub theme: Theme,
    /// Which speech engine transcribes audio. Unknown values in old
    /// settings files (for example "parakeet" from a removed backend) fall
    /// back to Whisper instead of failing the whole file.
    #[serde(deserialize_with = "lenient_backend")]
    pub stt_backend: SttBackend,
    /// Base URL of an OpenAI-compatible speech server, for example the one
    /// started by `fermion serve phonon-2`.
    pub stt_http_url: String,
    pub stt_http_model: String,
    /// Human-readable cpal device name. `None` uses whatever cpal reports as
    /// the system default at recording time.
    pub input_device: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyMode {
    PushToTalk,
    Toggle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SttBackend {
    Whisper,
    Http,
    /// Phonon-2 run by Freeflow itself (private Python env, hidden server).
    Phonon,
}

impl Default for SttBackend {
    fn default() -> Self {
        SttBackend::Whisper
    }
}

fn lenient_backend<'de, D>(d: D) -> Result<SttBackend, D::Error>
where
    D: Deserializer<'de>,
{
    // Accept any JSON value so a stale or hand-edited file (null, a number,
    // the removed "parakeet" backend) degrades to Whisper rather than
    // failing the whole settings parse.
    let raw = serde_json::Value::deserialize(d)?;
    Ok(match raw.as_str() {
        Some("http") => SttBackend::Http,
        Some("phonon") => SttBackend::Phonon,
        _ => SttBackend::Whisper,
    })
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ollama_base_url: "http://localhost:11434".into(),
            ollama_model: "llama3.1:8b".into(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.into(),
            hotkey: "ControlRight".into(),
            hotkey_mode: HotkeyMode::PushToTalk,
            auto_paste: true,
            copy_clipboard: true,
            whisper_model_path: None,
            whisper_language: "en".into(),
            show_indicator: true,
            llm_enabled: true,
            vocabulary: Vec::new(),
            theme: Theme::Dark,
            stt_backend: SttBackend::Whisper,
            stt_http_url: "http://127.0.0.1:8000/v1".into(),
            stt_http_model: "phonon-2".into(),
            input_device: None,
        }
    }
}

pub const DEFAULT_SYSTEM_PROMPT: &str = "You are a transcription post-processor. \
The user dictated the following text and a speech-to-text model transcribed it. \
Your job is to fix obvious transcription mistakes without changing the user's meaning, \
style, or word choice. Apply these fixes: \
convert spelled-out numbers like 'four oh one K' to '401K'; \
convert 'example dot com' to 'example.com'; \
fix homophones and punctuation; capitalize proper nouns and sentence starts; \
do NOT add content, do NOT answer questions, do NOT translate, do NOT change language. \
Respond with ONLY the cleaned text, no commentary, no quotes, no markdown.";

pub fn settings_file(app: &tauri::AppHandle) -> std::path::PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    std::fs::create_dir_all(&dir).ok();
    dir.join("settings.json")
}

pub fn load(app: &tauri::AppHandle) -> Settings {
    let path = settings_file(app);
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(error = ?e, path = ?path, "could not read settings file");
            }
            return Settings::default();
        }
    };
    match serde_json::from_str::<Settings>(&contents) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                error = ?e,
                path = ?path,
                "settings.json failed to parse; falling back to defaults"
            );
            // Back up the broken file so the user can inspect / recover it,
            // then overwrite with defaults so subsequent launches are clean.
            let backup = path.with_extension("json.broken");
            let _ = std::fs::rename(&path, &backup);
            Settings::default()
        }
    }
}

pub fn save(app: &tauri::AppHandle, s: &Settings) -> crate::error::Result<()> {
    let path = settings_file(app);
    let json = serde_json::to_string_pretty(s)?;
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_backend_values_fall_back_to_whisper() {
        for raw in [
            r#"{"stt_backend":"parakeet"}"#,
            r#"{"stt_backend":null}"#,
            r#"{"stt_backend":5}"#,
            r#"{}"#,
        ] {
            let s: Settings = serde_json::from_str(raw).unwrap();
            assert_eq!(s.stt_backend, SttBackend::Whisper, "{raw}");
        }
        let s: Settings = serde_json::from_str(r#"{"stt_backend":"http"}"#).unwrap();
        assert_eq!(s.stt_backend, SttBackend::Http);
    }

    #[test]
    fn legacy_parakeet_fields_do_not_break_loading() {
        let raw = r#"{"theme":"light","stt_backend":"parakeet","parakeet_model_dir":"C:/models/p","hotkey":"ControlLeft"}"#;
        let s: Settings = serde_json::from_str(raw).unwrap();
        assert_eq!(s.hotkey, "ControlLeft");
        assert_eq!(s.theme, Theme::Light);
        assert_eq!(s.stt_http_model, "phonon-2");
    }
}
