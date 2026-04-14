use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use super::{ChatMessage, LlmProvider, ModelInfo};

pub struct Ollama {
    base_url: String,
    client: reqwest::Client,
}

impl Ollama {
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("reqwest client");
        Self { base_url, client }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[derive(Debug, Deserialize)]
struct ModelsEnvelope {
    data: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}
#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: String,
}
#[derive(Debug, Deserialize)]
struct ChatEnvelope {
    choices: Vec<ChatChoice>,
}

#[async_trait]
impl LlmProvider for Ollama {
    async fn list_models(&self) -> anyhow::Result<Vec<ModelInfo>> {
        let url = format!("{}/v1/models", self.base_url);
        let resp = self.client.get(url).send().await?.error_for_status()?;
        let env: ModelsEnvelope = resp.json().await?;
        Ok(env.data)
    }

    async fn chat(&self, model: &str, messages: &[ChatMessage]) -> anyhow::Result<String> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = json!({
            "model": model,
            "messages": messages,
            "stream": false,
            "temperature": 0.1
        });
        let resp = self.client.post(url).json(&body).send().await?.error_for_status()?;
        let env: ChatEnvelope = resp.json().await?;
        Ok(env
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default()
            .trim()
            .to_string())
    }

    fn name(&self) -> &str {
        "ollama"
    }
}
