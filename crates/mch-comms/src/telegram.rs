//! Telegram integration via the Bot HTTP API (reqwest). No heavyweight SDK.

use crate::CommsError;
use serde::{Deserialize, Serialize};

/// Telegram bot configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    /// Default chat to report to.
    pub default_chat_id: i64,
    pub webhook_url: Option<String>,
    /// If true, `send_message` returns Ok without performing a network call.
    pub dry_run: bool,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            default_chat_id: 0,
            webhook_url: None,
            dry_run: true,
        }
    }
}

/// A minimal Telegram Bot API client.
pub struct TelegramClient {
    config: TelegramConfig,
    http: reqwest::Client,
    base_url: String,
    /// Messages sent while in dry-run (useful for tests / the overnight report).
    sent: std::sync::Mutex<Vec<(i64, String)>>,
}

impl TelegramClient {
    pub fn new(config: TelegramConfig) -> Result<Self, CommsError> {
        Ok(Self {
            config,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| CommsError::Telegram(e.to_string()))?,
            base_url: "https://api.telegram.org".to_string(),
            sent: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// Send a message to `chat_id`.
    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), CommsError> {
        self.sent
            .lock()
            .unwrap()
            .push((chat_id, text.to_string()));

        if self.config.dry_run || self.config.bot_token.is_empty() {
            return Ok(());
        }

        let url = format!("{}/bot{}/sendMessage", self.base_url, self.config.bot_token);
        let body = serde_json::json!({ "chat_id": chat_id, "text": text });
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| CommsError::Telegram(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(CommsError::Telegram(format!("HTTP {status}: {txt}")));
        }
        Ok(())
    }

    /// Send to the configured default chat.
    pub async fn send_report(&self, text: &str) -> Result<(), CommsError> {
        self.send_message(self.config.default_chat_id, text).await
    }

    /// Messages captured in dry-run mode.
    pub fn sent_messages(&self) -> Vec<(i64, String)> {
        self.sent.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dry_run_captures_message() {
        let client = TelegramClient::new(TelegramConfig {
            default_chat_id: 42,
            dry_run: true,
            ..Default::default()
        })
        .unwrap();
        client.send_report("morning report").await.unwrap();
        assert_eq!(client.sent_messages().len(), 1);
        assert_eq!(client.sent_messages()[0].0, 42);
    }
}
