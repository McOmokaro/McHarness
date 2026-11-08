//! Telegram integration

use crate::CommsError;
use serde::{Deserialize, Serialize};
use teloxide::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub webhook_url: Option<String>,
    pub allowed_users: Vec<i64>,
}

pub struct TelegramClient {
    bot: Bot,
    config: TelegramConfig,
}

impl TelegramClient {
    pub fn new(config: TelegramConfig) -> Result<Self, CommsError> {
        let bot = Bot::new(config.bot_token.clone());
        Ok(Self { bot, config })
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        self.bot.send_message(chat_id, text).await
            .map_err(|e| CommsError::Telegram(e.to_string()))?;
        Ok(())
    }

    pub async fn start_webhook(&self) -> Result<()> {
        if let Some(url) = &self.config.webhook_url {
            self.bot.set_webhook(url).await
                .map_err(|e| CommsError::Telegram(e.to_string()))?;
        }
        Ok(())
    }
}