//! Model layer: model-agnostic from birth. Any provider, any key, any model.

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model: String,
    pub key: String,
    pub prompt: String,
    pub input: String,
}

pub struct ModelRouter;

impl ModelRouter {
    pub async fn infer(&self, req: ModelRequest) -> Result<String> {
        // v0.0.1: local-first. Ollama speaks OpenAI-compatible JSON.
        // Hosted providers + leaderboard fallback land with the Key Farm telemetry.
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "model": req.model,
            "messages": [
                {"role": "system", "content": req.prompt},
                {"role": "user", "content": req.input}
            ]
        });

        let resp = client
            .post("http://localhost:11434/v1/chat/completions")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("model endpoint error: {}", resp.status());
        }

        #[derive(Deserialize)]
        struct Completion {
            choices: Vec<Choice>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: Message,
        }
        #[derive(Deserialize)]
        struct Message {
            content: String,
        }

        let completion: Completion = resp.json().await?;
        Ok(completion
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }
}
