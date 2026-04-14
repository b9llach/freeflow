pub mod ollama;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub object: Option<String>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, model: &str, messages: &[ChatMessage]) -> anyhow::Result<String>;
    async fn list_models(&self) -> anyhow::Result<Vec<ModelInfo>>;
    fn name(&self) -> &str;
}
