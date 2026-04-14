use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    pub vocabulary: Vec<String>,
    #[serde(default)]
    pub theme: Theme,
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
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(app: &tauri::AppHandle, s: &Settings) -> crate::error::Result<()> {
    let path = settings_file(app);
    let json = serde_json::to_string_pretty(s)?;
    std::fs::write(path, json)?;
    Ok(())
}
