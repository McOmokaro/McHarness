//! Binance API client

use crate::{TradeError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use hmac::{Hmac, Mac};
use sha2::Sha256;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceConfig {
    pub api_key: String,
    pub api_secret: String,
    pub base_url: String,
    pub testnet: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub balances: Vec<Balance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: String,
    pub locked: String,
}

pub struct BinanceClient {
    client: Client,
    config: BinanceConfig,
}

impl BinanceClient {
    pub fn new(config: BinanceConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        Self { client, config }
    }

    fn sign(&self, query: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.config.api_secret.as_bytes()).unwrap();
        mac.update(query.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    async fn get_signed(&self, endpoint: &str, params: &str) -> Result<reqwest::Response> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
        let query = format!("{}timestamp={}", params, timestamp);
        let signature = self.sign(&query);
        let url = format!("{}{}?{}&signature={}", self.config.base_url, endpoint, query, signature);
        
        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await
            .map_err(|e| TradeError::Binance(e.to_string()))?;
        Ok(response)
    }

    pub async fn account_info(&self) -> Result<AccountInfo> {
        let response = self.get_signed("/api/v3/account", "").await?;
        response.json().await.map_err(|e| TradeError::Binance(e.to_string()))
    }
}