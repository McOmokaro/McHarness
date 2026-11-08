//! Observation Engine — Accessibility Tree and Screenshot observation

use crate::cdp::{CdpClient, AccessibilityTree};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Observation configuration
#[derive(Debug, Clone)]
pub struct ObservationConfig {
    pub default_mode: ObservationMode,
}

impl Default for ObservationConfig {
    fn default() -> Self {
        Self {
            default_mode: ObservationMode::AccessibilityTree,
        }
    }
}

/// Observation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationMode {
    /// Accessibility tree only (primary)
    AccessibilityTree,
    /// Screenshot only (fallback)
    Screenshot,
    /// Both accessibility tree and screenshot
    Hybrid,
    /// Auto-select based on page content
    Auto,
}

/// Observation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationResult {
    pub mode: ObservationMode,
    pub accessibility_tree: Option<AccessibilityTree>,
    pub screenshot: Option<Vec<u8>>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Observation engine
pub struct ObservationEngine {
    cdp: Arc<CdpClient>,
    config: ObservationConfig,
}

impl ObservationEngine {
    pub fn new(cdp: Arc<CdpClient>, config: ObservationConfig) -> Self {
        Self { cdp, config }
    }

    /// Observe a page
    pub async fn observe(&self, page_id: uuid::Uuid) -> Result<ObservationResult> {
        self.observe_with_mode(page_id, self.config.default_mode).await
    }

    /// Observe with specific mode
    pub async fn observe_with_mode(&self, page_id: uuid::Uuid, mode: ObservationMode) -> Result<ObservationResult> {
        let timestamp = chrono::Utc::now();

        match mode {
            ObservationMode::AccessibilityTree => {
                let tree = self.cdp.get_accessibility_tree(page_id).await?;
                Ok(ObservationResult {
                    mode,
                    accessibility_tree: Some(tree),
                    screenshot: None,
                    timestamp,
                })
            }
            ObservationMode::Screenshot => {
                let screenshot = self.cdp.screenshot(page_id).await?;
                Ok(ObservationResult {
                    mode,
                    accessibility_tree: None,
                    screenshot: Some(screenshot),
                    timestamp,
                })
            }
            ObservationMode::Hybrid => {
                let tree = self.cdp.get_accessibility_tree(page_id).await?;
                let screenshot = self.cdp.screenshot(page_id).await?;
                Ok(ObservationResult {
                    mode,
                    accessibility_tree: Some(tree),
                    screenshot: Some(screenshot),
                    timestamp,
                })
            }
            ObservationMode::Auto => {
                // For now, default to hybrid - use Box::pin to avoid infinite recursion
                Box::pin(self.observe_with_mode(page_id, ObservationMode::Hybrid)).await
            }
        }
    }
}

use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observation_config_default() {
        let config = ObservationConfig::default();
        assert_eq!(config.default_mode, ObservationMode::AccessibilityTree);
    }
}