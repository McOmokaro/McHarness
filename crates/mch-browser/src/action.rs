//! Action Executor — GUI actions

use crate::cdp::{CdpClient, PageHandle};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// Action executor configuration
#[derive(Debug, Clone)]
pub struct ActionConfig {
    pub execution_timeout_secs: u64,
}

impl Default for ActionConfig {
    fn default() -> Self {
        Self {
            execution_timeout_secs: 30,
        }
    }
}

/// Execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub action_type: ActionType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Click,
    Type,
    Scroll,
    Navigate,
    Wait,
    Screenshot,
    ExecuteCode,
    Custom(String),
}

/// Action executor
pub struct ActionExecutor {
    cdp: Arc<CdpClient>,
    config: ActionConfig,
}

impl ActionExecutor {
    pub fn new(cdp: Arc<CdpClient>, config: ActionConfig) -> Self {
        Self { cdp, config }
    }

    /// Execute a click at coordinates
    pub async fn click(&self, page_id: uuid::Uuid, x: f64, y: f64) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();
        self.cdp.click(page_id, x, y).await?;
        Ok(ExecutionResult {
            success: true,
            output: format!("Clicked at ({}, {})", x, y),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            action_type: ActionType::Click,
        })
    }

    /// Execute type text
    pub async fn type_text(&self, page_id: uuid::Uuid, text: &str) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();
        self.cdp.type_text(page_id, text).await?;
        Ok(ExecutionResult {
            success: true,
            output: format!("Typed: {}", text),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            action_type: ActionType::Type,
        })
    }

    /// Execute scroll
    pub async fn scroll(&self, page_id: uuid::Uuid, dx: f64, dy: f64) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();
        self.cdp.scroll(page_id, dx, dy).await?;
        Ok(ExecutionResult {
            success: true,
            output: format!("Scrolled by ({}, {})", dx, dy),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            action_type: ActionType::Scroll,
        })
    }

    /// Execute navigate
    pub async fn navigate(&self, page_id: uuid::Uuid, url: &str) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();
        self.cdp.navigate(page_id, url).await?;
        Ok(ExecutionResult {
            success: true,
            output: format!("Navigated to {}", url),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            action_type: ActionType::Navigate,
        })
    }

    /// Execute wait
    pub async fn wait(&self, _page_id: uuid::Uuid, duration_ms: u64) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();
        tokio::time::sleep(tokio::time::Duration::from_millis(duration_ms)).await;
        Ok(ExecutionResult {
            success: true,
            output: format!("Waited {}ms", duration_ms),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            action_type: ActionType::Wait,
        })
    }

    /// Execute screenshot
    pub async fn screenshot(&self, page_id: uuid::Uuid) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();
        let screenshot = self.cdp.screenshot(page_id).await?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&screenshot);
        Ok(ExecutionResult {
            success: true,
            output: b64,
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            action_type: ActionType::Screenshot,
        })
    }
}

use uuid::Uuid;
use base64::{engine::general_purpose, Engine as _};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_config_default() {
        let config = ActionConfig::default();
        assert_eq!(config.execution_timeout_secs, 30);
    }
}