//! mch-browser — Native Computer-Use Agent (chromiumoxide 0.9)

pub mod cdp;
pub mod action;
pub mod observation;
pub mod permissions;

pub use cdp::{CdpClient, CdpConfig, PageHandle, CdpError};
pub use action::{ActionExecutor, ActionConfig, ExecutionResult, ActionType};
pub use observation::{ObservationEngine, ObservationConfig, ObservationMode, ObservationResult};
pub use permissions::{PermissionManager, PermissionConfig, PermissionRequest, PermissionDecision, RiskLevel};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cdp_config_default() {
        let config = CdpConfig::default();
        assert!(config.headless);
        assert_eq!(config.window_size, (1920, 1080));
    }

    #[tokio::test]
    async fn test_permission_risk_levels() {
        assert!(RiskLevel::Critical > RiskLevel::High);
        assert!(RiskLevel::High > RiskLevel::Medium);
        assert!(RiskLevel::Medium > RiskLevel::Low);
    }
}