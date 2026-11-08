//! Permission Manager — OmniPilot-style granular permissions

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Permission configuration
#[derive(Debug, Clone)]
pub struct PermissionConfig {
    pub default_risk_level: RiskLevel,
    pub auto_approve_low_risk: bool,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            default_risk_level: RiskLevel::Medium,
            auto_approve_low_risk: true,
        }
    }
}

/// Risk levels (OmniPilot-style)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Permission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: uuid::Uuid,
    pub action_type: String,
    pub description: String,
    pub risk_level: RiskLevel,
    pub metadata: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Permission decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionDecision {
    Approved,
    Denied(String),
    Pending,
}

/// Permission manager
pub struct PermissionManager {
    config: PermissionConfig,
    pending_requests: Arc<RwLock<HashMap<uuid::Uuid, PermissionRequest>>>,
    decisions: Arc<RwLock<HashMap<uuid::Uuid, PermissionDecision>>>,
}

impl PermissionManager {
    pub fn new(config: PermissionConfig) -> Self {
        Self {
            config,
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            decisions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Request permission for an action
    pub async fn request_permission(&self, request: PermissionRequest) -> Result<PermissionDecision> {
        let id = request.id;
        
        // Auto-approve low risk if configured
        if self.config.auto_approve_low_risk && request.risk_level <= RiskLevel::Low {
            info!("Auto-approving low-risk action: {}", request.action_type);
            self.decisions.write().await.insert(id, PermissionDecision::Approved);
            return Ok(PermissionDecision::Approved);
        }

        // Store pending request
        self.pending_requests.write().await.insert(id, request.clone());
        
        // For now, auto-approve based on risk level
        let decision = match request.risk_level {
            RiskLevel::Low => PermissionDecision::Approved,
            RiskLevel::Medium => PermissionDecision::Approved,
            RiskLevel::High => PermissionDecision::Denied("High risk action requires manual approval".to_string()),
            RiskLevel::Critical => PermissionDecision::Denied("Critical risk action denied".to_string()),
        };

        self.decisions.write().await.insert(id, decision.clone());
        self.pending_requests.write().await.remove(&id);

        Ok(decision)
    }

    /// Get decision for a request
    pub async fn get_decision(&self, id: uuid::Uuid) -> Option<PermissionDecision> {
        self.decisions.read().await.get(&id).cloned()
    }

    /// Check if action is allowed without explicit request
    pub async fn is_allowed(&self, action_type: &str, risk_level: RiskLevel) -> bool {
        if risk_level <= RiskLevel::Low && self.config.auto_approve_low_risk {
            return true;
        }
        if risk_level <= RiskLevel::Medium {
            return true;
        }
        false
    }
}

use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }

    #[tokio::test]
    async fn test_auto_approve_low_risk() {
        let config = PermissionConfig {
            auto_approve_low_risk: true,
            ..Default::default()
        };
        let manager = PermissionManager::new(config);
        
        let request = PermissionRequest {
            id: uuid::Uuid::new_v4(),
            action_type: "click".to_string(),
            description: "Click a button".to_string(),
            risk_level: RiskLevel::Low,
            metadata: serde_json::json!({}),
            timestamp: chrono::Utc::now(),
        };

        let decision = manager.request_permission(request).await.unwrap();
        assert!(matches!(decision, PermissionDecision::Approved));
    }
}