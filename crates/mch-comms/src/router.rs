//! Deterministic routing and model classification for Low-Fuel architecture

use crate::CommsError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub route: RouteType,
    pub confidence: f32,
    pub reasoning: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteType {
    Deterministic,
    LocalModel,
    HeavyModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub category: MessageCategory,
    pub confidence: f32,
    pub suggested_route: RouteType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageCategory {
    Spam,
    Lead,
    MeetingRequest,
    Notification,
    Other,
}

pub struct DeterministicRouter {
    rules: Vec<RoutingRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoutingRule {
    pattern: String,
    category: MessageCategory,
    route: RouteType,
}

impl DeterministicRouter {
    pub fn new() -> Self {
        Self {
            rules: vec![
                RoutingRule { pattern: "meeting".to_string(), category: MessageCategory::MeetingRequest, route: RouteType::Deterministic },
                RoutingRule { pattern: "schedule".to_string(), category: MessageCategory::MeetingRequest, route: RouteType::Deterministic },
                RoutingRule { pattern: "calendar".to_string(), category: MessageCategory::MeetingRequest, route: RouteType::Deterministic },
                RoutingRule { pattern: "spam".to_string(), category: MessageCategory::Spam, route: RouteType::Deterministic },
                RoutingRule { pattern: "unsubscribe".to_string(), category: MessageCategory::Spam, route: RouteType::Deterministic },
                RoutingRule { pattern: "interested".to_string(), category: MessageCategory::Lead, route: RouteType::LocalModel },
                RoutingRule { pattern: "pricing".to_string(), category: MessageCategory::Lead, route: RouteType::LocalModel },
                RoutingRule { pattern: "demo".to_string(), category: MessageCategory::Lead, route: RouteType::LocalModel },
            ],
        }
    }

    pub fn route(&self, message: &str) -> RouteDecision {
        let lower = message.to_lowercase();
        for rule in &self.rules {
            if lower.contains(&rule.pattern) {
                return RouteDecision {
                    route: rule.route,
                    confidence: 0.9,
                    reasoning: format!("Matched pattern: {}", rule.pattern),
                };
            }
        }
        RouteDecision {
            route: RouteType::LocalModel,
            confidence: 0.5,
            reasoning: "No deterministic match, defaulting to local model".to_string(),
        }
    }
}

pub struct ModelClassifier {
    // In production, would use fastembed or similar for embeddings
    // For now, simple keyword-based classification
    spam_keywords: Vec<String>,
    lead_keywords: Vec<String>,
    meeting_keywords: Vec<String>,
}

impl ModelClassifier {
    pub fn new() -> Self {
        Self {
            spam_keywords: vec!["unsubscribe".to_string(), "spam".to_string(), "promotion".to_string(), "offer".to_string()],
            lead_keywords: vec!["interested".to_string(), "pricing".to_string(), "demo".to_string(), "quote".to_string(), "purchase".to_string()],
            meeting_keywords: vec!["meeting".to_string(), "schedule".to_string(), "calendar".to_string(), "appointment".to_string(), "call".to_string()],
        }
    }

    pub fn classify(&self, message: &str) -> ClassificationResult {
        let lower = message.to_lowercase();
        
        let spam_score = self.spam_keywords.iter().filter(|k| lower.contains(k)).count() as f32;
        let lead_score = self.lead_keywords.iter().filter(|k| lower.contains(k)).count() as f32;
        let meeting_score = self.meeting_keywords.iter().filter(|k| lower.contains(k)).count() as f32;
        
        let total = spam_score + lead_score + meeting_score + 1.0;
        
        if spam_score > lead_score && spam_score > meeting_score {
            ClassificationResult {
                category: MessageCategory::Spam,
                confidence: spam_score / total,
                suggested_route: RouteType::Deterministic,
            }
        } else if lead_score > meeting_score {
            ClassificationResult {
                category: MessageCategory::Lead,
                confidence: lead_score / total,
                suggested_route: RouteType::LocalModel,
            }
        } else if meeting_score > 0.0 {
            ClassificationResult {
                category: MessageCategory::MeetingRequest,
                confidence: meeting_score / total,
                suggested_route: RouteType::Deterministic,
            }
        } else {
            ClassificationResult {
                category: MessageCategory::Other,
                confidence: 0.3,
                suggested_route: RouteType::LocalModel,
            }
        }
    }
}