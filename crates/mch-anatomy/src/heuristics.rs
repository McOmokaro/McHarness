//! Behavioral Heuristics — Friction responses and decision-making patterns
//!
//! This module defines how personas respond to friction, blocks, ambiguity,
//! and failure. Heuristics provide reusable behavioral patterns that can be
//! composed and adapted based on context.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Type of friction encountered during task execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrictionType {
    /// Hard blocker — cannot proceed without external resolution
    Block,
    /// Ambiguity — multiple valid paths, unclear requirements
    Ambiguity,
    /// Interpersonal or priority conflict
    Conflict,
    /// Execution failure — action didn't produce expected result
    Failure,
    /// Time pressure or deadline-driven friction
    Delay,
    /// Resource constraint (compute, budget, access)
    ResourceConstraint,
    /// Quality standard not met
    QualityGate,
}

/// Behavioral response to friction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrictionResponse {
    /// Push through with force/authority
    ForceThrough,
    /// Negotiate a path forward
    Negotiate,
    /// Analyze root cause before acting
    AnalyzeRootCause,
    /// Wait and observe for more information
    WaitAndObserve,
    /// Explore alternative approaches
    ExploreAlternatives,
    /// Find a workaround
    FindWorkaround,
    /// Collaborate with others
    Collaborate,
    /// Verify assumptions before proceeding
    VerifyAssumptions,
    /// Decide quickly with available info
    DecideFast,
    /// Clarify requirements
    Clarify,
    /// Gather more data
    GatherMoreData,
    /// Model uncertainty explicitly
    ModelUncertainty,
    /// Probe deeper into the problem
    ProbeDeeper,
    /// Demand evidence for claims
    DemandEvidence,
    /// Confront the blocker directly
    Confront,
    /// Mediate between parties
    Mediate,
    /// Understand all perspectives
    UnderstandPerspectives,
    /// Challenge claims critically
    ChallengeClaims,
    /// Document and escalate
    DocumentAndEscalate,
    /// De-escalate tension
    DeEscalate,
    /// Apologize and retry
    ApologizeAndRetry,
    /// Retry aggressively
    RetryAggressive,
    /// Root cause analysis
    RootCauseAnalysis,
    /// Learn from failure
    LearnFromFailure,
    /// Calm recovery
    CalmRecovery,
    /// Supportive recovery
    SupportiveRecovery,
    /// Verify before retry
    VerifyBeforeRetry,
    /// Retry with adjustment
    RetryWithAdjustment,
    /// Optimize process
    OptimizeProcess,
    /// Wait patiently
    WaitPatiently,
    /// Escalate to higher authority
    Escalate,
    /// Investigate cause
    InvestigateCause,
    /// Reprioritize work
    Reprioritize,
    /// Communicate proactively
    CommunicateProactively,
    /// Seek consensus
    SeekConsensus,
    /// Compromise
    Compromise,
    /// Demand accountability
    DemandAccountability,
}

impl FrictionResponse {
    /// Get the category of this response
    pub fn category(&self) -> ResponseCategory {
        match self {
            FrictionResponse::ForceThrough
            | FrictionResponse::Confront
            | FrictionResponse::Escalate
            | FrictionResponse::RetryAggressive
            | FrictionResponse::DecideFast => ResponseCategory::Assertive,

            FrictionResponse::Negotiate
            | FrictionResponse::Collaborate
            | FrictionResponse::Mediate
            | FrictionResponse::Compromise
            | FrictionResponse::SeekConsensus
            | FrictionResponse::CommunicateProactively
            | FrictionResponse::ApologizeAndRetry
            | FrictionResponse::SupportiveRecovery
            | FrictionResponse::UnderstandPerspectives => ResponseCategory::Collaborative,

            FrictionResponse::AnalyzeRootCause
            | FrictionResponse::RootCauseAnalysis
            | FrictionResponse::GatherMoreData
            | FrictionResponse::ProbeDeeper
            | FrictionResponse::VerifyAssumptions
            | FrictionResponse::DemandEvidence
            | FrictionResponse::VerifyBeforeRetry
            | FrictionResponse::InvestigateCause
            | FrictionResponse::Clarify
            | FrictionResponse::DocumentAndEscalate
            | FrictionResponse::LearnFromFailure
            | FrictionResponse::ChallengeClaims => ResponseCategory::Analytical,

            FrictionResponse::WaitAndObserve
            | FrictionResponse::WaitPatiently
            | FrictionResponse::ModelUncertainty
            | FrictionResponse::DeEscalate
            | FrictionResponse::CalmRecovery
            | FrictionResponse::OptimizeProcess
            | FrictionResponse::Reprioritize
            | FrictionResponse::ExploreAlternatives
            | FrictionResponse::FindWorkaround
            | FrictionResponse::RetryWithAdjustment => ResponseCategory::Adaptive,
        }
    }

    /// Get response intensity (0.0 = passive, 1.0 = aggressive)
    pub fn intensity(&self) -> f32 {
        match self {
            FrictionResponse::ForceThrough => 1.0,
            FrictionResponse::Confront => 0.95,
            FrictionResponse::Escalate => 0.9,
            FrictionResponse::RetryAggressive => 0.85,
            FrictionResponse::DecideFast => 0.8,
            FrictionResponse::ChallengeClaims => 0.75,
            FrictionResponse::DemandAccountability => 0.7,
            FrictionResponse::DemandEvidence => 0.65,
            FrictionResponse::DocumentAndEscalate => 0.6,
            FrictionResponse::AnalyzeRootCause => 0.55,
            FrictionResponse::RootCauseAnalysis => 0.55,
            FrictionResponse::ProbeDeeper => 0.5,
            FrictionResponse::InvestigateCause => 0.5,
            FrictionResponse::GatherMoreData => 0.45,
            FrictionResponse::VerifyAssumptions => 0.4,
            FrictionResponse::VerifyBeforeRetry => 0.4,
            FrictionResponse::LearnFromFailure => 0.35,
            FrictionResponse::Negotiate => 0.3,
            FrictionResponse::Mediate => 0.3,
            FrictionResponse::Collaborate => 0.25,
            FrictionResponse::SeekConsensus => 0.2,
            FrictionResponse::Compromise => 0.2,
            FrictionResponse::CommunicateProactively => 0.15,
            FrictionResponse::FindWorkaround => 0.1,
            FrictionResponse::ExploreAlternatives => 0.1,
            FrictionResponse::OptimizeProcess => 0.1,
            FrictionResponse::RetryWithAdjustment => 0.1,
            FrictionResponse::Reprioritize => 0.05,
            FrictionResponse::DeEscalate => 0.05,
            FrictionResponse::WaitAndObserve => 0.0,
            FrictionResponse::WaitPatiently => 0.0,
            FrictionResponse::ModelUncertainty => 0.0,
            FrictionResponse::CalmRecovery => 0.0,
            FrictionResponse::SupportiveRecovery => 0.0,
            FrictionResponse::ApologizeAndRetry => 0.0,
            FrictionResponse::Clarify => 0.0,
            FrictionResponse::UnderstandPerspectives => 0.0,
        }
    }

    /// Get recommended follow-up responses
    pub fn follow_ups(&self) -> Vec<FrictionResponse> {
        match self {
            FrictionResponse::ForceThrough => vec![FrictionResponse::VerifyBeforeRetry, FrictionResponse::DocumentAndEscalate],
            FrictionResponse::Negotiate => vec![FrictionResponse::Compromise, FrictionResponse::DocumentAndEscalate],
            FrictionResponse::AnalyzeRootCause => vec![FrictionResponse::OptimizeProcess, FrictionResponse::RetryWithAdjustment],
            FrictionResponse::WaitAndObserve => vec![FrictionResponse::ProbeDeeper, FrictionResponse::DecideFast],
            FrictionResponse::ExploreAlternatives => vec![FrictionResponse::DecideFast, FrictionResponse::FindWorkaround],
            FrictionResponse::FindWorkaround => vec![FrictionResponse::VerifyBeforeRetry, FrictionResponse::OptimizeProcess],
            FrictionResponse::Collaborate => vec![FrictionResponse::SeekConsensus, FrictionResponse::CommunicateProactively],
            FrictionResponse::VerifyAssumptions => vec![FrictionResponse::GatherMoreData, FrictionResponse::DecideFast],
            FrictionResponse::DecideFast => vec![FrictionResponse::VerifyBeforeRetry, FrictionResponse::MonitorOutcome],
            FrictionResponse::Clarify => vec![FrictionResponse::GatherMoreData, FrictionResponse::DecideFast],
            FrictionResponse::GatherMoreData => vec![FrictionResponse::AnalyzeRootCause, FrictionResponse::DecideFast],
            FrictionResponse::ModelUncertainty => vec![FrictionResponse::ProbeDeeper, FrictionResponse::DecideFast],
            FrictionResponse::ProbeDeeper => vec![FrictionResponse::AnalyzeRootCause, FrictionResponse::DecideFast],
            FrictionResponse::DemandEvidence => vec![FrictionResponse::VerifyAssumptions, FrictionResponse::ChallengeClaims],
            FrictionResponse::Confront => vec![FrictionResponse::DeEscalate, FrictionResponse::DocumentAndEscalate],
            FrictionResponse::Mediate => vec![FrictionResponse::Compromise, FrictionResponse::SeekConsensus],
            FrictionResponse::UnderstandPerspectives => vec![FrictionResponse::Mediate, FrictionResponse::SeekConsensus],
            FrictionResponse::ChallengeClaims => vec![FrictionResponse::DemandEvidence, FrictionResponse::VerifyAssumptions],
            FrictionResponse::DocumentAndEscalate => vec![FrictionResponse::Escalate, FrictionResponse::WaitPatiently],
            FrictionResponse::DeEscalate => vec![FrictionResponse::Mediate, FrictionResponse::Collaborate],
            FrictionResponse::ApologizeAndRetry => vec![FrictionResponse::RetryWithAdjustment, FrictionResponse::CommunicateProactively],
            FrictionResponse::RetryAggressive => vec![FrictionResponse::AnalyzeRootCause, FrictionResponse::Escalate],
            FrictionResponse::RootCauseAnalysis => vec![FrictionResponse::OptimizeProcess, FrictionResponse::LearnFromFailure],
            FrictionResponse::LearnFromFailure => vec![FrictionResponse::OptimizeProcess, FrictionResponse::RetryWithAdjustment],
            FrictionResponse::CalmRecovery => vec![FrictionResponse::RetryWithAdjustment, FrictionResponse::CommunicateProactively],
            FrictionResponse::SupportiveRecovery => vec![FrictionResponse::Collaborate, FrictionResponse::CommunicateProactively],
            FrictionResponse::VerifyBeforeRetry => vec![FrictionResponse::RetryWithAdjustment, FrictionResponse::GatherMoreData],
            FrictionResponse::RetryWithAdjustment => vec![FrictionResponse::VerifyBeforeRetry, FrictionResponse::OptimizeProcess],
            FrictionResponse::OptimizeProcess => vec![FrictionResponse::MonitorOutcome, FrictionResponse::LearnFromFailure],
            FrictionResponse::WaitPatiently => vec![FrictionResponse::CommunicateProactively, FrictionResponse::ProbeDeeper],
            FrictionResponse::Escalate => vec![FrictionResponse::WaitPatiently, FrictionResponse::DocumentAndEscalate],
            FrictionResponse::InvestigateCause => vec![FrictionResponse::AnalyzeRootCause, FrictionResponse::LearnFromFailure],
            FrictionResponse::Reprioritize => vec![FrictionResponse::CommunicateProactively, FrictionResponse::DecideFast],
            FrictionResponse::CommunicateProactively => vec![FrictionResponse::SeekConsensus, FrictionResponse::Collaborate],
            FrictionResponse::SeekConsensus => vec![FrictionResponse::Compromise, FrictionResponse::DocumentAndEscalate],
            FrictionResponse::Compromise => vec![FrictionResponse::MonitorOutcome, FrictionResponse::CommunicateProactively],
            FrictionResponse::DemandAccountability => vec![FrictionResponse::Escalate, FrictionResponse::DocumentAndEscalate],
        }
    }

    /// Monitor outcome as a follow-up (implicit)
    fn monitor_outcome(&self) -> FrictionResponse {
        FrictionResponse::WaitAndObserve
    }
}

/// Response category for grouping similar behaviors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseCategory {
    Assertive,
    Collaborative,
    Analytical,
    Adaptive,
}

/// Context for heuristic selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicContext {
    /// Domain of the task (e.g., "trading", "debugging", "scheduling")
    pub domain: String,
    /// Keywords extracted from the current situation
    pub keywords: Vec<String>,
    /// Pressure level (0.0 = no pressure, 1.0 = extreme urgency)
    pub pressure_level: f32,
    /// Whether consensus is required
    pub requires_consensus: bool,
    /// Current friction type if any
    pub friction_type: Option<FrictionType>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Default for HeuristicContext {
    fn default() -> Self {
        Self {
            domain: "general".to_string(),
            keywords: vec![],
            pressure_level: 0.5,
            requires_consensus: false,
            friction_type: None,
            metadata: HashMap::new(),
        }
    }
}

/// Outcome of applying a heuristic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicOutcome {
    /// The response that was selected
    pub response: FrictionResponse,
    /// Confidence in this response (0.0 - 1.0)
    pub confidence: f32,
    /// Expected time to resolution
    pub expected_resolution_time: Option<std::time::Duration>,
    /// Risk assessment
    pub risk_level: RiskLevel,
    /// Recommended follow-up actions
    pub follow_ups: Vec<FrictionResponse>,
    /// Reasoning trace
    pub reasoning: String,
}

/// Risk level for heuristic outcomes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Behavioral heuristic — a reusable pattern for handling friction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralHeuristic {
    /// Unique identifier
    pub id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Description of when this heuristic applies
    pub description: String,
    /// Friction types this heuristic addresses
    pub applies_to: Vec<FrictionType>,
    /// Required context conditions
    pub preconditions: Vec<String>,
    /// The core response pattern
    pub response: FrictionResponse,
    /// Confidence in this heuristic (0.0 - 1.0)
    pub base_confidence: f32,
    /// Domain specializations
    pub domains: Vec<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Success rate from historical application
    pub success_rate: f32,
    /// Number of times applied
    pub application_count: u64,
}

impl BehavioralHeuristic {
    /// Create a new heuristic
    pub fn new(
        name: String,
        description: String,
        applies_to: Vec<FrictionType>,
        response: FrictionResponse,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            applies_to,
            preconditions: vec![],
            response,
            base_confidence: 0.7,
            domains: vec![],
            tags: vec![],
            success_rate: 0.0,
            application_count: 0,
        }
    }

    /// Check if this heuristic applies to a context
    pub fn applies(&self, context: &HeuristicContext) -> bool {
        // Check friction type match
        if let Some(friction) = context.friction_type {
            if !self.applies_to.contains(&friction) {
                return false;
            }
        }

        // Check domain match (if heuristic has domains specified)
        if !self.domains.is_empty() && !self.domains.contains(&context.domain) {
            return false;
        }

        // Check preconditions (simplified - in practice would evaluate expressions)
        for precondition in &self.preconditions {
            // Simple keyword-based precondition check
            if !context.keywords.iter().any(|k| k.contains(precondition)) {
                return false;
            }
        }

        true
    }

    /// Calculate confidence for a specific context
    pub fn confidence_for(&self, context: &HeuristicContext) -> f32 {
        let mut confidence = self.base_confidence;

        // Adjust for pressure level
        let intensity = self.response.intensity();
        if context.pressure_level > 0.7 && intensity > 0.7 {
            confidence += 0.15;
        } else if context.pressure_level < 0.3 && intensity < 0.3 {
            confidence += 0.1;
        }

        // Adjust for consensus requirement
        if context.requires_consensus {
            if self.response.category() == ResponseCategory::Collaborative {
                confidence += 0.15;
            } else if self.response.category() == ResponseCategory::Assertive {
                confidence -= 0.2;
            }
        }

        // Historical success rate boost
        if self.application_count > 10 {
            confidence += (self.success_rate - 0.5) * 0.2;
        }

        confidence.clamp(0.0, 1.0)
    }

    /// Record application outcome for learning
    pub fn record_outcome(&mut self, success: bool) {
        self.application_count += 1;
        let alpha = 0.1; // Exponential moving average factor
        self.success_rate = self.success_rate * (1.0 - alpha) + (if success { 1.0 } else { 0.0 }) * alpha;
    }
}

/// Heuristic engine — selects and applies behavioral heuristics
#[derive(Debug, Default)]
pub struct HeuristicEngine {
    heuristics: Vec<BehavioralHeuristic>,
}

impl HeuristicEngine {
    /// Create a new engine with default heuristics
    pub fn new() -> Self {
        let mut engine = Self {
            heuristics: Vec::new(),
        };
        engine.register_defaults();
        engine
    }

    /// Register default heuristics
    fn register_defaults(&mut self) {
        // Assertive heuristics
        self.register(BehavioralHeuristic::new(
            "Force Through Block".to_string(),
            "When blocked by a hard constraint, push through with authority or escalation".to_string(),
            vec![FrictionType::Block],
            FrictionResponse::ForceThrough,
        ));

        self.register(BehavioralHeuristic::new(
            "Confront Conflict".to_string(),
            "Address interpersonal conflict directly and immediately".to_string(),
            vec![FrictionType::Conflict],
            FrictionResponse::Confront,
        ));

        self.register(BehavioralHeuristic::new(
            "Decide Under Pressure".to_string(),
            "When time is critical, make the best decision with available information".to_string(),
            vec![FrictionType::Delay, FrictionType::Ambiguity],
            FrictionResponse::DecideFast,
        ));

        // Collaborative heuristics
        self.register(BehavioralHeuristic::new(
            "Negotiate Block".to_string(),
            "When blocked, negotiate with the blocker for a path forward".to_string(),
            vec![FrictionType::Block],
            FrictionResponse::Negotiate,
        ));

        self.register(BehavioralHeuristic::new(
            "Mediate Conflict".to_string(),
            "Facilitate resolution between conflicting parties".to_string(),
            vec![FrictionType::Conflict],
            FrictionResponse::Mediate,
        ));

        self.register(BehavioralHeuristic::new(
            "Seek Consensus".to_string(),
            "When ambiguity affects multiple stakeholders, build agreement".to_string(),
            vec![FrictionType::Ambiguity],
            FrictionResponse::SeekConsensus,
        ));

        // Analytical heuristics
        self.register(BehavioralHeuristic::new(
            "Root Cause Analysis".to_string(),
            "When failure occurs, systematically identify the underlying cause".to_string(),
            vec![FrictionType::Failure, FrictionType::QualityGate],
            FrictionResponse::RootCauseAnalysis,
        ));

        self.register(BehavioralHeuristic::new(
            "Analyze Before Acting".to_string(),
            "When blocked by complexity, analyze root cause before attempting solutions".to_string(),
            vec![FrictionType::Block, FrictionType::Ambiguity],
            FrictionResponse::AnalyzeRootCause,
        ));

        self.register(BehavioralHeuristic::new(
            "Gather More Data".to_string(),
            "When facing ambiguity, collect more information before deciding".to_string(),
            vec![FrictionType::Ambiguity],
            FrictionResponse::GatherMoreData,
        ));

        self.register(BehavioralHeuristic::new(
            "Verify Assumptions".to_string(),
            "Before proceeding, explicitly verify critical assumptions".to_string(),
            vec![FrictionType::Ambiguity, FrictionType::QualityGate],
            FrictionResponse::VerifyAssumptions,
        ));

        // Adaptive heuristics
        self.register(BehavioralHeuristic::new(
            "Find Workaround".to_string(),
            "When blocked, find an alternative path to the goal".to_string(),
            vec![FrictionType::Block, FrictionType::ResourceConstraint],
            FrictionResponse::FindWorkaround,
        ));

        self.register(BehavioralHeuristic::new(
            "Wait and Observe".to_string(),
            "When uncertain and time permits, observe for more signal".to_string(),
            vec![FrictionType::Ambiguity, FrictionType::Delay],
            FrictionResponse::WaitAndObserve,
        ));

        self.register(BehavioralHeuristic::new(
            "Retry with Adjustment".to_string(),
            "After failure, adjust approach and retry".to_string(),
            vec![FrictionType::Failure],
            FrictionResponse::RetryWithAdjustment,
        ));

        self.register(BehavioralHeuristic::new(
            "Explore Alternatives".to_string(),
            "When the primary path is blocked, systematically explore alternatives".to_string(),
            vec![FrictionType::Block, FrictionType::ResourceConstraint],
            FrictionResponse::ExploreAlternatives,
        ));

        self.register(BehavioralHeuristic::new(
            "Optimize Process".to_string(),
            "When delays or quality issues recur, improve the underlying process".to_string(),
            vec![FrictionType::Delay, FrictionType::QualityGate],
            FrictionResponse::OptimizeProcess,
        ));

        // Specialized heuristics
        self.register(BehavioralHeuristic::new(
            "Model Uncertainty".to_string(),
            "Explicitly model and quantify uncertainty before committing".to_string(),
            vec![FrictionType::Ambiguity],
            FrictionResponse::ModelUncertainty,
        ));

        self.register(BehavioralHeuristic::new(
            "Learn From Failure".to_string(),
            "Extract maximum learning from failures to prevent recurrence".to_string(),
            vec![FrictionType::Failure],
            FrictionResponse::LearnFromFailure,
        ));

        self.register(BehavioralHeuristic::new(
            "De-escalate Tension".to_string(),
            "When conflict is heated, reduce temperature before resolving".to_string(),
            vec![FrictionType::Conflict],
            FrictionResponse::DeEscalate,
        ));

        self.register(BehavioralHeuristic::new(
            "Communicate Proactively".to_string(),
            "When delays occur, communicate status and plans to stakeholders early".to_string(),
            vec![FrictionType::Delay],
            FrictionResponse::CommunicateProactively,
        ));
    }

    /// Register a heuristic
    pub fn register(&mut self, heuristic: BehavioralHeuristic) {
        self.heuristics.push(heuristic);
    }

    /// Select the best heuristic for a context
    pub fn select(&self, context: &HeuristicContext) -> Option<HeuristicOutcome> {
        let applicable: Vec<_> = self.heuristics
            .iter()
            .filter(|h| h.applies(context))
            .collect();

        if applicable.is_empty() {
            return None;
        }

        // Select highest confidence
        let best = applicable.iter()
            .max_by(|a, b| {
                a.confidence_for(context)
                    .partial_cmp(&b.confidence_for(context))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })?;

        let confidence = best.confidence_for(context);
        let risk = self.assess_risk(&best.response, context);

        Some(HeuristicOutcome {
            response: best.response,
            confidence,
            expected_resolution_time: self.estimate_resolution_time(&best.response, context),
            risk_level: risk,
            follow_ups: best.response.follow_ups(),
            reasoning: format!(
                "Selected '{}' (confidence: {:.2}) for {} friction in {} domain",
                best.name, confidence,
                context.friction_type.map(|f| format!("{:?}", f)).unwrap_or("unknown".to_string()),
                context.domain
            ),
        })
    }

    /// Get all applicable heuristics ranked by confidence
    pub fn ranked(&self, context: &HeuristicContext) -> Vec<(&BehavioralHeuristic, f32)> {
        let mut ranked: Vec<_> = self.heuristics
            .iter()
            .filter(|h| h.applies(context))
            .map(|h| (h, h.confidence_for(context)))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked
    }

    /// Assess risk level for a response in context
    fn assess_risk(&self, response: &FrictionResponse, context: &HeuristicContext) -> RiskLevel {
        let intensity = response.intensity();
        let pressure = context.pressure_level;

        match (intensity, pressure) {
            (i, _) if i > 0.8 => RiskLevel::High,
            (i, p) if i > 0.6 && p > 0.7 => RiskLevel::High,
            (i, _) if i > 0.5 => RiskLevel::Medium,
            (_, p) if p > 0.8 => RiskLevel::Medium,
            _ => RiskLevel::Low,
        }
    }

    /// Estimate resolution time for a response
    fn estimate_resolution_time(&self, response: &FrictionResponse, context: &HeuristicContext) -> Option<std::time::Duration> {
        let base_minutes = match response.category() {
            ResponseCategory::Assertive => 5,
            ResponseCategory::Collaborative => 30,
            ResponseCategory::Analytical => 60,
            ResponseCategory::Adaptive => 15,
        };

        let pressure_multiplier = if context.pressure_level > 0.7 { 0.5 } else { 1.0 };
        Some(std::time::Duration::from_secs((base_minutes as f32 * pressure_multiplier * 60.0) as u64))
    }

    /// Record outcome for learning
    pub fn record_outcome(&mut self, heuristic_name: &str, success: bool) {
        if let Some(h) = self.heuristics.iter_mut().find(|h| h.name == heuristic_name) {
            h.record_outcome(success);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_friction_response_category() {
        assert_eq!(FrictionResponse::ForceThrough.category(), ResponseCategory::Assertive);
        assert_eq!(FrictionResponse::Negotiate.category(), ResponseCategory::Collaborative);
        assert_eq!(FrictionResponse::AnalyzeRootCause.category(), ResponseCategory::Analytical);
        assert_eq!(FrictionResponse::FindWorkaround.category(), ResponseCategory::Adaptive);
    }

    #[test]
    fn test_friction_response_intensity() {
        assert!(FrictionResponse::ForceThrough.intensity() > FrictionResponse::Negotiate.intensity());
        assert!(FrictionResponse::AnalyzeRootCause.intensity() > FrictionResponse::WaitAndObserve.intensity());
        assert_eq!(FrictionResponse::WaitAndObserve.intensity(), 0.0);
        assert_eq!(FrictionResponse::ForceThrough.intensity(), 1.0);
    }

    #[test]
    fn test_friction_response_follow_ups() {
        let follow_ups = FrictionResponse::ForceThrough.follow_ups();
        assert!(follow_ups.contains(&FrictionResponse::VerifyBeforeRetry));
        assert!(follow_ups.contains(&FrictionResponse::DocumentAndEscalate));
    }

    #[test]
    fn test_heuristic_engine_defaults() {
        let engine = HeuristicEngine::new();
        assert!(!engine.heuristics.is_empty());
        assert!(engine.heuristics.len() >= 18);
    }

    #[test]
    fn test_heuristic_selection_block() {
        let engine = HeuristicEngine::new();
        let context = HeuristicContext {
            domain: "general".to_string(),
            keywords: vec![],
            pressure_level: 0.5,
            requires_consensus: false,
            friction_type: Some(FrictionType::Block),
            metadata: HashMap::new(),
        };

        let outcome = engine.select(&context);
        assert!(outcome.is_some());
        let outcome = outcome.unwrap();
        assert!(matches!(outcome.response, FrictionResponse::ForceThrough | FrictionResponse::Negotiate | FrictionResponse::FindWorkaround | FrictionResponse::AnalyzeRootCause));
    }

    #[test]
    fn test_heuristic_selection_conflict_consensus() {
        let engine = HeuristicEngine::new();
        let context = HeuristicContext {
            domain: "general".to_string(),
            keywords: vec![],
            pressure_level: 0.3,
            requires_consensus: true,
            friction_type: Some(FrictionType::Conflict),
            metadata: HashMap::new(),
        };

        let outcome = engine.select(&context);
        assert!(outcome.is_some());
        let outcome = outcome.unwrap();
        // Should prefer collaborative responses when consensus required
        assert!(matches!(outcome.response, FrictionResponse::Mediate | FrictionResponse::SeekConsensus | FrictionResponse::Negotiate | FrictionResponse::DeEscalate));
    }

    #[test]
    fn test_heuristic_selection_high_pressure() {
        let engine = HeuristicEngine::new();
        let context = HeuristicContext {
            domain: "trading".to_string(),
            keywords: vec!["urgent".to_string()],
            pressure_level: 0.9,
            requires_consensus: false,
            friction_type: Some(FrictionType::Delay),
            metadata: HashMap::new(),
        };

        let outcome = engine.select(&context);
        assert!(outcome.is_some());
        let outcome = outcome.unwrap();
        // Should prefer assertive/adaptive responses under high pressure
        assert!(matches!(outcome.response, FrictionResponse::DecideFast | FrictionResponse::ForceThrough | FrictionResponse::ExploreAlternatives | FrictionResponse::FindWorkaround));
    }

    #[test]
    fn test_behavioral_heuristic_applies() {
        let heuristic = BehavioralHeuristic::new(
            "Test".to_string(),
            "Test heuristic".to_string(),
            vec![FrictionType::Block],
            FrictionResponse::ForceThrough,
        );

        let context = HeuristicContext {
            domain: "general".to_string(),
            keywords: vec![],
            pressure_level: 0.5,
            requires_consensus: false,
            friction_type: Some(FrictionType::Block),
            metadata: HashMap::new(),
        };
        assert!(heuristic.applies(&context));

        let context2 = HeuristicContext {
            friction_type: Some(FrictionType::Conflict),
            ..context
        };
        assert!(!heuristic.applies(&context2));
    }

    #[test]
    fn test_heuristic_confidence_adjustment() {
        let mut heuristic = BehavioralHeuristic::new(
            "Test".to_string(),
            "Test heuristic".to_string(),
            vec![FrictionType::Block],
            FrictionResponse::ForceThrough,
        );

        let context_low_pressure = HeuristicContext {
            domain: "general".to_string(),
            keywords: vec![],
            pressure_level: 0.1,
            requires_consensus: false,
            friction_type: Some(FrictionType::Block),
            metadata: HashMap::new(),
        };

        let context_high_pressure = HeuristicContext {
            pressure_level: 0.9,
            ..context_low_pressure.clone()
        };

        let confidence_low = heuristic.confidence_for(&context_low_pressure);
        let confidence_high = heuristic.confidence_for(&context_high_pressure);

        // ForceThrough is high intensity, should be more confident under high pressure
        assert!(confidence_high > confidence_low);
    }

    #[test]
    fn test_heuristic_learning() {
        let mut heuristic = BehavioralHeuristic::new(
            "Test".to_string(),
            "Test heuristic".to_string(),
            vec![FrictionType::Block],
            FrictionResponse::ForceThrough,
        );

        assert_eq!(heuristic.success_rate, 0.0);
        assert_eq!(heuristic.application_count, 0);

        heuristic.record_outcome(true);
        assert_eq!(heuristic.application_count, 1);
        assert!(heuristic.success_rate > 0.0);

        heuristic.record_outcome(false);
        assert_eq!(heuristic.application_count, 2);
    }
}