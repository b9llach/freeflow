use serde::{Deserialize, Serialize};
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
    pub stt_backend: SttBackend,
    pub parakeet_model_dir: Option<PathBuf>,
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
    Parakeet,
}

impl Default for SttBackend {
    fn default() -> Self {
        SttBackend::Whisper
    }
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
            parakeet_model_dir: None,
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
