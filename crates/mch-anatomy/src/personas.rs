//! Cognitive Personas — Archetypal interaction styles for human emulation
//!
//! This module defines cognitive personas that model distinct human archetypes
//! for LLM agent emulation. Each persona encapsulates a coherent set of behavioral
//! traits, communication patterns, and decision-making heuristics.

use crate::heuristics::{FrictionResponse, FrictionType, HeuristicContext};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Core personality trait that defines a persona's behavioral tendency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonaTrait {
    /// Assertive, competitive, high risk tolerance
    Aggressive,
    /// Polite, consensus-seeking, relationship-oriented
    Polite,
    /// Detail-oriented, systematic, quality-focused
    Meticulous,
    /// Calm, analytical, emotionally regulated
    Composed,
    /// Curious, exploratory, open to novelty
    Inquisitive,
    /// Pragmatic, outcome-focused, efficiency-driven
    Pragmatic,
    /// Empathetic, supportive, harmony-seeking
    Empathetic,
    /// Skeptical, critical, verification-oriented
    Skeptical,
}

impl PersonaTrait {
    /// Get the default friction response for this trait
    pub fn default_friction_response(&self, friction_type: FrictionType) -> FrictionResponse {
        match (self, friction_type) {
            (PersonaTrait::Aggressive, FrictionType::Block) => FrictionResponse::ForceThrough,
            (PersonaTrait::Aggressive, FrictionType::Ambiguity) => FrictionResponse::DecideFast,
            (PersonaTrait::Aggressive, FrictionType::Conflict) => FrictionResponse::Confront,
            (PersonaTrait::Aggressive, FrictionType::Failure) => FrictionResponse::RetryAggressive,
            (PersonaTrait::Aggressive, FrictionType::Delay) => FrictionResponse::Escalate,

            (PersonaTrait::Polite, FrictionType::Block) => FrictionResponse::Negotiate,
            (PersonaTrait::Polite, FrictionType::Ambiguity) => FrictionResponse::Clarify,
            (PersonaTrait::Polite, FrictionType::Conflict) => FrictionResponse::Mediate,
            (PersonaTrait::Polite, FrictionType::Failure) => FrictionResponse::ApologizeAndRetry,
            (PersonaTrait::Polite, FrictionType::Delay) => FrictionResponse::WaitPatiently,

            (PersonaTrait::Meticulous, FrictionType::Block) => FrictionResponse::AnalyzeRootCause,
            (PersonaTrait::Meticulous, FrictionType::Ambiguity) => FrictionResponse::GatherMoreData,
            (PersonaTrait::Meticulous, FrictionType::Conflict) => FrictionResponse::DocumentAndEscalate,
            (PersonaTrait::Meticulous, FrictionType::Failure) => FrictionResponse::RootCauseAnalysis,
            (PersonaTrait::Meticulous, FrictionType::Delay) => FrictionResponse::OptimizeProcess,

            (PersonaTrait::Composed, FrictionType::Block) => FrictionResponse::WaitAndObserve,
            (PersonaTrait::Composed, FrictionType::Ambiguity) => FrictionResponse::ModelUncertainty,
            (PersonaTrait::Composed, FrictionType::Conflict) => FrictionResponse::DeEscalate,
            (PersonaTrait::Composed, FrictionType::Failure) => FrictionResponse::CalmRecovery,
            (PersonaTrait::Composed, FrictionType::Delay) => FrictionResponse::WaitPatiently,

            (PersonaTrait::Inquisitive, FrictionType::Block) => FrictionResponse::ExploreAlternatives,
            (PersonaTrait::Inquisitive, FrictionType::Ambiguity) => FrictionResponse::ProbeDeeper,
            (PersonaTrait::Inquisitive, FrictionType::Conflict) => FrictionResponse::UnderstandPerspectives,
            (PersonaTrait::Inquisitive, FrictionType::Failure) => FrictionResponse::LearnFromFailure,
            (PersonaTrait::Inquisitive, FrictionType::Delay) => FrictionResponse::InvestigateCause,

            (PersonaTrait::Pragmatic, FrictionType::Block) => FrictionResponse::FindWorkaround,
            (PersonaTrait::Pragmatic, FrictionType::Ambiguity) => FrictionResponse::DecideFast,
            (PersonaTrait::Pragmatic, FrictionType::Conflict) => FrictionResponse::Compromise,
            (PersonaTrait::Pragmatic, FrictionType::Failure) => FrictionResponse::RetryWithAdjustment,
            (PersonaTrait::Pragmatic, FrictionType::Delay) => FrictionResponse::Reprioritize,

            (PersonaTrait::Empathetic, FrictionType::Block) => FrictionResponse::Collaborate,
            (PersonaTrait::Empathetic, FrictionType::Ambiguity) => FrictionResponse::SeekConsensus,
            (PersonaTrait::Empathetic, FrictionType::Conflict) => FrictionResponse::Mediate,
            (PersonaTrait::Empathetic, FrictionType::Failure) => FrictionResponse::SupportiveRecovery,
            (PersonaTrait::Empathetic, FrictionType::Delay) => FrictionResponse::CommunicateProactively,

            (PersonaTrait::Skeptical, FrictionType::Block) => FrictionResponse::VerifyAssumptions,
            (PersonaTrait::Skeptical, FrictionType::Ambiguity) => FrictionResponse::DemandEvidence,
            (PersonaTrait::Skeptical, FrictionType::Conflict) => FrictionResponse::ChallengeClaims,
            (PersonaTrait::Skeptical, FrictionType::Failure) => FrictionResponse::VerifyBeforeRetry,
            (PersonaTrait::Skeptical, FrictionType::Delay) => FrictionResponse::DemandAccountability,
        }
    }

    /// Get trait weight for persona matching (0.0 - 1.0)
    pub fn weight(&self) -> f32 {
        match self {
            PersonaTrait::Aggressive => 0.9,
            PersonaTrait::Polite => 0.7,
            PersonaTrait::Meticulous => 0.85,
            PersonaTrait::Composed => 0.8,
            PersonaTrait::Inquisitive => 0.75,
            PersonaTrait::Pragmatic => 0.9,
            PersonaTrait::Empathetic => 0.7,
            PersonaTrait::Skeptical => 0.85,
        }
    }
}

/// Configuration for a cognitive persona
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaConfig {
    /// Unique identifier
    pub id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Description of the persona's archetype
    pub description: String,
    /// Primary personality traits (ordered by dominance)
    pub primary_traits: Vec<PersonaTrait>,
    /// Secondary personality traits
    pub secondary_traits: Vec<PersonaTrait>,
    /// Communication style parameters
    pub communication: CommunicationStyle,
    /// Decision-making parameters
    pub decision_making: DecisionMakingStyle,
    /// Domain specializations
    pub domains: Vec<String>,
    /// Trigger keywords that activate this persona
    pub triggers: Vec<String>,
    /// Confidence threshold for persona selection (0.0 - 1.0)
    pub selection_threshold: f32,
}

impl Default for PersonaConfig {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Default".to_string(),
            description: "Generic persona".to_string(),
            primary_traits: vec![PersonaTrait::Pragmatic],
            secondary_traits: vec![],
            communication: CommunicationStyle::default(),
            decision_making: DecisionMakingStyle::default(),
            domains: vec![],
            triggers: vec![],
            selection_threshold: 0.5,
        }
    }
}

/// Communication style parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationStyle {
    /// Verbosity level (0.0 = terse, 1.0 = verbose)
    pub verbosity: f32,
    /// Formality level (0.0 = casual, 1.0 = formal)
    pub formality: f32,
    /// Directness level (0.0 = indirect, 1.0 = direct)
    pub directness: f32,
    /// Empathy expression (0.0 = clinical, 1.0 = warm)
    pub empathy: f32,
    /// Technical depth (0.0 = high-level, 1.0 = detailed)
    pub technical_depth: f32,
    /// Preferred pronouns/style markers
    pub style_markers: Vec<String>,
}

impl Default for CommunicationStyle {
    fn default() -> Self {
        Self {
            verbosity: 0.5,
            formality: 0.5,
            directness: 0.5,
            empathy: 0.5,
            technical_depth: 0.5,
            style_markers: vec![],
        }
    }
}

/// Decision-making style parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionMakingStyle {
    /// Risk tolerance (0.0 = risk-averse, 1.0 = risk-seeking)
    pub risk_tolerance: f32,
    /// Speed vs accuracy preference (0.0 = thorough, 1.0 = fast)
    pub speed_vs_accuracy: f32,
    /// Consensus requirement (0.0 = autonomous, 1.0 = consensus-driven)
    pub consensus_requirement: f32,
    /// Data requirement before deciding (0.0 = intuition, 1.0 = data-driven)
    pub data_driven: f32,
    /// Reversibility preference (0.0 = commit, 1.0 = keep options open)
    pub reversibility_preference: f32,
}

impl Default for DecisionMakingStyle {
    fn default() -> Self {
        Self {
            risk_tolerance: 0.5,
            speed_vs_accuracy: 0.5,
            consensus_requirement: 0.5,
            data_driven: 0.5,
            reversibility_preference: 0.5,
        }
    }
}

/// A cognitive persona — an archetypal interaction style
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitivePersona {
    /// Configuration
    pub config: PersonaConfig,
    /// Runtime state
    #[serde(skip)]
    pub state: PersonaState,
}

/// Runtime state for a persona
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersonaState {
    /// Current active friction responses
    pub active_responses: HashMap<String, FrictionResponse>,
    /// Interaction history summary
    pub interaction_count: u64,
    /// Success rate in current session
    pub success_rate: f32,
    /// Last activation timestamp
    pub last_activated: Option<chrono::DateTime<chrono::Utc>>,
}

impl CognitivePersona {
    /// Create a new persona from config
    pub fn new(config: PersonaConfig) -> Self {
        Self {
            config,
            state: PersonaState::default(),
        }
    }

    /// Create the aggressive trader persona
    pub fn aggressive_trader() -> Self {
        Self::new(PersonaConfig {
            id: Uuid::new_v4(),
            name: "Aggressive Trader".to_string(),
            description: "High-frequency, risk-tolerant, decisive under pressure. Optimizes for speed and alpha capture.".to_string(),
            primary_traits: vec![PersonaTrait::Aggressive, PersonaTrait::Pragmatic],
            secondary_traits: vec![PersonaTrait::Skeptical],
            communication: CommunicationStyle {
                verbosity: 0.3,
                formality: 0.2,
                directness: 0.95,
                empathy: 0.1,
                technical_depth: 0.8,
                style_markers: vec!["execute".into(), "position".into(), "risk/reward".into(), "stop-loss".into()],
            },
            decision_making: DecisionMakingStyle {
                risk_tolerance: 0.9,
                speed_vs_accuracy: 0.9,
                consensus_requirement: 0.05,
                data_driven: 0.7,
                reversibility_preference: 0.3,
            },
            domains: vec!["trading".into(), "risk_management".into(), "execution".into()],
            triggers: vec!["trade".into(), "position".into(), "market".into(), "volatility".into(), "entry".into(), "exit".into()],
            selection_threshold: 0.6,
        })
    }

    /// Create the polite meeting scheduler persona
    pub fn polite_scheduler() -> Self {
        Self::new(PersonaConfig {
            id: Uuid::new_v4(),
            name: "Polite Meeting Scheduler".to_string(),
            description: "Consensus-building, relationship-oriented, excels at coordination and stakeholder alignment.".to_string(),
            primary_traits: vec![PersonaTrait::Polite, PersonaTrait::Empathetic],
            secondary_traits: vec![PersonaTrait::Pragmatic],
            communication: CommunicationStyle {
                verbosity: 0.7,
                formality: 0.8,
                directness: 0.3,
                empathy: 0.9,
                technical_depth: 0.4,
                style_markers: vec!["would you be available".into(), "kindly".into(), "appreciate".into(), "align".into()],
            },
            decision_making: DecisionMakingStyle {
                risk_tolerance: 0.2,
                speed_vs_accuracy: 0.3,
                consensus_requirement: 0.9,
                data_driven: 0.6,
                reversibility_preference: 0.8,
            },
            domains: vec!["scheduling".into(), "coordination".into(), "stakeholder_management".into()],
            triggers: vec!["meeting".into(), "schedule".into(), "calendar".into(), "availability".into(), "coordinate".into()],
            selection_threshold: 0.55,
        })
    }

    /// Create the meticulous bug hunter persona
    pub fn meticulous_bug_hunter() -> Self {
        Self::new(PersonaConfig {
            id: Uuid::new_v4(),
            name: "Meticulous Bug Hunter".to_string(),
            description: "Systematic, detail-obsessed, root-cause driven. Leaves no stone unturned in debugging.".to_string(),
            primary_traits: vec![PersonaTrait::Meticulous, PersonaTrait::Skeptical],
            secondary_traits: vec![PersonaTrait::Inquisitive, PersonaTrait::Composed],
            communication: CommunicationStyle {
                verbosity: 0.8,
                formality: 0.7,
                directness: 0.7,
                empathy: 0.3,
                technical_depth: 0.95,
                style_markers: vec!["root cause".into(), "reproduction".into(), "stack trace".into(), "hypothesis".into(), "verify".into()],
            },
            decision_making: DecisionMakingStyle {
                risk_tolerance: 0.1,
                speed_vs_accuracy: 0.1,
                consensus_requirement: 0.2,
                data_driven: 0.95,
                reversibility_preference: 0.9,
            },
            domains: vec!["debugging".into(), "root_cause_analysis".into(), "testing".into(), "quality_assurance".into()],
            triggers: vec!["bug".into(), "error".into(), "fail".into(), "debug".into(), "trace".into(), "reproduce".into()],
            selection_threshold: 0.65,
        })
    }

    /// Create the composed analyst persona
    pub fn composed_analyst() -> Self {
        Self::new(PersonaConfig {
            id: Uuid::new_v4(),
            name: "Composed Analyst".to_string(),
            description: "Calm under pressure, models uncertainty, maintains strategic perspective during chaos.".to_string(),
            primary_traits: vec![PersonaTrait::Composed, PersonaTrait::Inquisitive],
            secondary_traits: vec![PersonaTrait::Pragmatic, PersonaTrait::Skeptical],
            communication: CommunicationStyle {
                verbosity: 0.6,
                formality: 0.7,
                directness: 0.6,
                empathy: 0.5,
                technical_depth: 0.8,
                style_markers: vec!["probability".into(), "scenario".into(), "uncertainty".into(), "evidence suggests".into()],
            },
            decision_making: DecisionMakingStyle {
                risk_tolerance: 0.4,
                speed_vs_accuracy: 0.4,
                consensus_requirement: 0.3,
                data_driven: 0.8,
                reversibility_preference: 0.7,
            },
            domains: vec!["analysis".into(), "strategy".into(), "risk_assessment".into(), "planning".into()],
            triggers: vec!["analyze".into(), "assess".into(), "strategy".into(), "forecast".into(), "scenario".into()],
            selection_threshold: 0.5,
        })
    }

    /// Get the default friction response for this persona's primary trait
    pub fn default_friction_response(&self, friction_type: FrictionType) -> FrictionResponse {
        if let Some(primary) = self.config.primary_traits.first() {
            primary.default_friction_response(friction_type)
        } else {
            FrictionResponse::WaitAndObserve
        }
    }

    /// Score how well this persona matches a given context (0.0 - 1.0)
    pub fn match_score(&self, context: &HeuristicContext) -> f32 {
        let mut score = 0.0;
        let mut factors = 0;

        // Domain match
        for domain in &self.config.domains {
            if context.domain.contains(domain) {
                score += 1.0;
                factors += 1;
            }
        }

        // Trigger keyword match
        for trigger in &self.config.triggers {
            if context.keywords.iter().any(|k| k.contains(trigger)) {
                score += 0.5;
                factors += 1;
            }
        }

        // Trait alignment with context pressure
        if context.pressure_level > 0.7 {
            if self.config.primary_traits.contains(&PersonaTrait::Composed)
                || self.config.primary_traits.contains(&PersonaTrait::Aggressive) {
                score += 0.5;
            }
        } else if context.pressure_level < 0.3 {
            if self.config.primary_traits.contains(&PersonaTrait::Meticulous)
                || self.config.primary_traits.contains(&PersonaTrait::Inquisitive) {
                score += 0.5;
            }
        }
        factors += 1;

        // Required consensus alignment
        if context.requires_consensus {
            score += self.config.decision_making.consensus_requirement;
        } else {
            score += 1.0 - self.config.decision_making.consensus_requirement;
        }
        factors += 1;

        if factors == 0 {
            0.0
        } else {
            (score / factors as f32).min(1.0)
        }
    }

    /// Generate a response prefix based on communication style
    pub fn response_prefix(&self, intent: &str) -> String {
        let style = &self.config.communication;
        let mut prefix = String::new();

        if style.formality > 0.7 {
            prefix.push_str("Certainly, ");
        } else if style.formality < 0.3 {
            prefix.push_str("Sure, ");
        }

        if style.directness > 0.7 {
            prefix.push_str(&format!("I'll {} ", intent));
        } else {
            prefix.push_str(&format!("Let me {} ", intent));
        }

        prefix
    }
}

/// Registry of cognitive personas
#[derive(Debug, Default)]
pub struct PersonaRegistry {
    personas: HashMap<Uuid, CognitivePersona>,
    by_name: HashMap<String, Uuid>,
}

impl PersonaRegistry {
    /// Create a new registry with default personas
    pub fn new() -> Self {
        let mut registry = Self {
            personas: HashMap::new(),
            by_name: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Register default personas
    fn register_defaults(&mut self) {
        let defaults = vec![
            CognitivePersona::aggressive_trader(),
            CognitivePersona::polite_scheduler(),
            CognitivePersona::meticulous_bug_hunter(),
            CognitivePersona::composed_analyst(),
        ];

        for persona in defaults {
            let name = persona.config.name.clone();
            let id = persona.config.id;
            self.by_name.insert(name, id);
            self.personas.insert(id, persona);
        }
    }

    /// Register a custom persona
    pub fn register(&mut self, persona: CognitivePersona) {
        let name = persona.config.name.clone();
        let id = persona.config.id;
        self.by_name.insert(name, id);
        self.personas.insert(id, persona);
    }

    /// Get a persona by ID
    pub fn get(&self, id: &Uuid) -> Option<&CognitivePersona> {
        self.personas.get(id)
    }

    /// Get a persona by name
    pub fn get_by_name(&self, name: &str) -> Option<&CognitivePersona> {
        self.by_name.get(name).and_then(|id| self.personas.get(id))
    }

    /// Get mutable persona by ID
    pub fn get_mut(&mut self, id: &Uuid) -> Option<&mut CognitivePersona> {
        self.personas.get_mut(id)
    }

    /// List all persona names
    pub fn list(&self) -> Vec<String> {
        self.by_name.keys().cloned().collect()
    }

    /// Select the best persona for a given context
    pub fn select_best(&self, context: &HeuristicContext) -> Option<&CognitivePersona> {
        self.personas
            .values()
            .filter(|p| p.match_score(context) >= p.config.selection_threshold)
            .max_by(|a, b| {
                a.match_score(context)
                    .partial_cmp(&b.match_score(context))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get all personas sorted by match score for a context
    pub fn ranked_for_context(&self, context: &HeuristicContext) -> Vec<(&CognitivePersona, f32)> {
        let mut ranked: Vec<_> = self.personas
            .values()
            .map(|p| (p, p.match_score(context)))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_personas_registered() {
        let registry = PersonaRegistry::new();
        assert_eq!(registry.list().len(), 4);
        assert!(registry.get_by_name("Aggressive Trader").is_some());
        assert!(registry.get_by_name("Polite Meeting Scheduler").is_some());
        assert!(registry.get_by_name("Meticulous Bug Hunter").is_some());
        assert!(registry.get_by_name("Composed Analyst").is_some());
    }

    #[test]
    fn test_aggressive_trader_friction_response() {
        let persona = CognitivePersona::aggressive_trader();
        assert_eq!(
            persona.default_friction_response(crate::heuristics::FrictionType::Block),
            crate::heuristics::FrictionResponse::ForceThrough
        );
        assert_eq!(
            persona.default_friction_response(crate::heuristics::FrictionType::Conflict),
            crate::heuristics::FrictionResponse::Confront
        );
    }

    #[test]
    fn test_polite_scheduler_friction_response() {
        let persona = CognitivePersona::polite_scheduler();
        assert_eq!(
            persona.default_friction_response(crate::heuristics::FrictionType::Block),
            crate::heuristics::FrictionResponse::Negotiate
        );
        assert_eq!(
            persona.default_friction_response(crate::heuristics::FrictionType::Conflict),
            crate::heuristics::FrictionResponse::Mediate
        );
    }

    #[test]
    fn test_meticulous_bug_hunter_friction_response() {
        let persona = CognitivePersona::meticulous_bug_hunter();
        assert_eq!(
            persona.default_friction_response(crate::heuristics::FrictionType::Block),
            crate::heuristics::FrictionResponse::AnalyzeRootCause
        );
        assert_eq!(
            persona.default_friction_response(crate::heuristics::FrictionType::Failure),
            crate::heuristics::FrictionResponse::RootCauseAnalysis
        );
    }

    #[test]
    fn test_persona_match_score() {
        let registry = PersonaRegistry::new();
        let context = HeuristicContext {
            domain: "trading".to_string(),
            keywords: vec!["trade".to_string(), "position".to_string()],
            pressure_level: 0.8,
            requires_consensus: false,
            friction_type: None,
            metadata: std::collections::HashMap::new(),
        };

        let ranked = registry.ranked_for_context(&context);
        assert!(ranked.len() >= 4);
        // Aggressive trader should rank highest for trading context
        assert_eq!(ranked[0].0.config.name, "Aggressive Trader");
    }

    #[test]
    fn test_custom_persona_registration() {
        let mut registry = PersonaRegistry::new();
        let initial_count = registry.list().len();

        let custom = CognitivePersona::new(PersonaConfig {
            name: "Custom Persona".to_string(),
            ..Default::default()
        });
        registry.register(custom);

        assert_eq!(registry.list().len(), initial_count + 1);
        assert!(registry.get_by_name("Custom Persona").is_some());
    }
}