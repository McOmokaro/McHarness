//! Consensus Kernel — Multi-agent consensus for security decisions
//!
//! Forged from Spotlight (CMUL8/spotlight) Consensus Kernel v1
//! Implements PRD §8 semantics: independence check + tiered promotion

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;
use uuid::Uuid;

/// Valid modalities that count as independent corroborators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceModality {
    /// Deterministic static analysis finding (e.g., Semgrep rule hit)
    StaticAnalysisFact,
    /// Dynamic reproduction of the vulnerability (strongest single item)
    DynamicReproduction,
    /// Different model family, OR fresh context + different prompt scaffold
    IndependentAgent,
    /// SAST/DAST/AI-scanner importer hit
    ExternalSignal,
    /// Planner-inferred rule fired (ranked at parity with external_signal)
    PlannerInferred,
}

/// Static fact classes that short-circuit to verified
const STATIC_FACT_CLASSES: &[&str] = &["secrets", "hardcoded-secret"];

/// Evidence item — one corroborator record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    /// Modality of this evidence
    pub modality: EvidenceModality,
    /// Origin metadata; independence check keys on (modality, model_family, context_id)
    #[serde(default)]
    pub origin: serde_json::Value,
    /// Result: confirmed | not-confirmed | not-applicable | sanitized | inconclusive
    pub result: String,
    /// Arbitrary detail/rationale
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

impl EvidenceItem {
    /// Independence key for deduplication: (modality, model_family, context_id)
    pub fn independence_key(&self) -> (EvidenceModality, Option<String>, Option<String>) {
        let model_family = self.origin.get("model_family").and_then(|v| v.as_str()).map(|s| s.to_string());
        let context_id = self.origin.get("context_id").and_then(|v| v.as_str()).map(|s| s.to_string());
        (self.modality, model_family, context_id)
    }

    /// Does this item positively assert a vulnerability?
    pub fn is_positive(&self) -> bool {
        match self.modality {
            EvidenceModality::DynamicReproduction => self.result == "confirmed",
            EvidenceModality::StaticAnalysisFact => {
                !matches!(self.result.as_str(), "sanitized" | "not-confirmed" | "refuted")
            }
            // Independent agents / external signals / planner_inferred
            _ => matches!(self.result.as_str(), "confirmed" | "positive" | "yes" | "flagged"),
        }
    }

    /// Does this item negatively assert (sanitized/not-vuln)?
    pub fn is_negative(&self) -> bool {
        matches!(self.result.as_str(), "sanitized" | "not-confirmed" | "refuted" | "negative" | "no")
    }
}

/// Deduplicate evidence items by independence key
pub fn dedupe_independent(evidence: &[EvidenceItem]) -> Vec<EvidenceItem> {
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    for item in evidence {
        if !matches!(item.modality, EvidenceModality::StaticAnalysisFact | EvidenceModality::DynamicReproduction | EvidenceModality::IndependentAgent | EvidenceModality::ExternalSignal | EvidenceModality::PlannerInferred) {
            continue;
        }
        let key = item.independence_key();
        if seen.insert(key) {
            kept.push(item.clone());
        }
    }
    kept
}

/// Check if evidence items disagree (at least one positive, one negative)
pub fn has_disagreement(evidence: &[EvidenceItem]) -> bool {
    let pos = evidence.iter().any(|i| i.is_positive());
    let neg = evidence.iter().any(|i| i.is_negative());
    pos && neg
}

/// Consensus tier decision
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    Verified,
    HighConfidence,
    NeedsReview,
    Held,
}

/// Consensus verdict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierDecision {
    pub tier: Tier,
    pub confidence: f64,
    pub rationale: String,
    pub independent_corroborators: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjudication: Option<serde_json::Value>,
}

/// Consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Minimum agents required for consensus
    pub min_agents: usize,
    /// Threshold ratio for consensus (e.g., 0.67 for 2/3 majority)
    pub threshold: f64,
    /// Require unanimous consent for OutOfScope verdicts
    pub require_unanimous_deny: bool,
    /// Maximum time to wait for votes (milliseconds)
    pub timeout_ms: u64,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            min_agents: 3,
            threshold: 0.67,
            require_unanimous_deny: true,
            timeout_ms: 5000,
        }
    }
}

/// Consensus errors
#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("quorum not met: {voted}/{required} agents voted")]
    QuorumNotMet { voted: usize, required: usize },
    #[error("consensus failed: {0}")]
    ConsensusFailed(String),
    #[error("adjudication failed: {0}")]
    AdjudicationFailed(String),
}

/// Evidence item for consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVote {
    pub agent_id: String,
    pub verdict: Verdict,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
    pub timestamp_ms: u64,
    pub rationale: Option<String>,
}

impl AgentVote {
    pub fn new(
        agent_id: String,
        verdict: Verdict,
        signing_key: &ed25519_dalek::SigningKey,
        rationale: Option<String>,
    ) -> Self {
        use ed25519_dalek::Signer;
        let public_key = signing_key.verifying_key().to_bytes().to_vec();
        let payload = Self::vote_payload(&agent_id, verdict, rationale.as_deref());
        let signature = signing_key.sign(&payload).to_bytes().to_vec();

        Self {
            agent_id,
            verdict,
            signature,
            public_key,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            rationale,
        }
    }

    fn vote_payload(agent_id: &str, verdict: Verdict, rationale: Option<&str>) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(agent_id.as_bytes());
        payload.push(verdict as u8);
        if let Some(r) = rationale {
            payload.extend_from_slice(r.as_bytes());
        }
        payload
    }

    pub fn verify(&self) -> bool {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        let pk: [u8; 32] = self.public_key.as_slice().try_into().unwrap_or([0u8; 32]);
        let Ok(public_key) = VerifyingKey::from_bytes(&pk) else {
            return false;
        };
        let sig: [u8; 64] = self.signature.as_slice().try_into().unwrap_or([0u8; 64]);
        let payload = Self::vote_payload(&self.agent_id, self.verdict, self.rationale.as_deref());
        public_key.verify(&payload, &Signature::from_bytes(&sig)).is_ok()
    }
}

/// Scope verdict for consensus
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    InScope,
    OutOfScope,
    Uncertain,
}

/// Consensus kernel — coordinates multi-agent agreement on security decisions
pub struct ConsensusKernel {
    config: ConsensusConfig,
    // Registered agent verifying keys
    agents: std::collections::BTreeMap<String, ed25519_dalek::VerifyingKey>,
}

impl ConsensusKernel {
    /// Create a new consensus kernel with registered agents
    pub fn new(config: ConsensusConfig, agents: Vec<(String, ed25519_dalek::VerifyingKey)>) -> Self {
        let agents = agents.into_iter().collect();
        Self { config, agents }
    }

    /// Register a new agent
    pub fn register_agent(&mut self, agent_id: String, verifying_key: ed25519_dalek::VerifyingKey) {
        self.agents.insert(agent_id, verifying_key);
    }

    /// Run consensus on a scope decision
    pub fn decide(
        &self,
        host: &str,
        local_verdict: Verdict,
        votes: Vec<AgentVote>,
    ) -> Result<ConsensusVerdict, ConsensusError> {
        // Verify all votes
        let mut valid_votes = Vec::new();
        for vote in votes {
            if !vote.verify() {
                return Err(ConsensusError::ConsensusFailed(format!(
                    "Invalid signature from agent {}",
                    vote.agent_id
                )));
            }
            if !self.agents.contains_key(&vote.agent_id) {
                return Err(ConsensusError::ConsensusFailed(format!(
                    "Unregistered agent {}",
                    vote.agent_id
                )));
            }
            valid_votes.push(vote);
        }

        // Check quorum
        if valid_votes.len() < self.config.min_agents {
            return Err(ConsensusError::QuorumNotMet {
                voted: valid_votes.len(),
                required: self.config.min_agents,
            });
        }

        // Tally votes
        let mut tally = std::collections::BTreeMap::new();
        for vote in &valid_votes {
            *tally.entry(vote.verdict).or_insert(0) += 1;
        }

        let total = valid_votes.len() as f64;
        let in_scope_count = *tally.get(&Verdict::InScope).unwrap_or(&0) as f64;
        let out_scope_count = *tally.get(&Verdict::OutOfScope).unwrap_or(&0) as f64;

        // Determine consensus
        let consensus_reached;
        let verdict;
        let mut dissenters = Vec::new();

        if out_scope_count > 0.0 && self.config.require_unanimous_deny {
            // Deny-wins: any OutOfScope vote with unanimous deny requirement
            if out_scope_count / total >= 1.0 {
                consensus_reached = true;
                verdict = Verdict::OutOfScope;
            } else {
                consensus_reached = false;
                verdict = local_verdict; // Fall back to local
                for vote in &valid_votes {
                    if vote.verdict == Verdict::OutOfScope {
                        dissenters.push(vote.agent_id.clone());
                    }
                }
            }
        } else if in_scope_count / total >= self.config.threshold {
            consensus_reached = true;
            verdict = Verdict::InScope;
            for vote in &valid_votes {
                if vote.verdict != Verdict::InScope {
                    dissenters.push(vote.agent_id.clone());
                }
            }
        } else if out_scope_count / total >= self.config.threshold {
            consensus_reached = true;
            verdict = Verdict::OutOfScope;
            for vote in &valid_votes {
                if vote.verdict != Verdict::OutOfScope {
                    dissenters.push(vote.agent_id.clone());
                }
            }
        } else {
            consensus_reached = false;
            verdict = Verdict::Uncertain;
            for vote in &valid_votes {
                dissenters.push(vote.agent_id.clone());
            }
        }

        Ok(ConsensusVerdict {
            verdict,
            participant_count: valid_votes.len(),
            vote_tally: tally,
            consensus_reached,
            votes: valid_votes,
            dissenters,
        })
    }
}

/// Result of a consensus round
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusVerdict {
    pub verdict: Verdict,
    pub participant_count: usize,
    pub vote_tally: std::collections::BTreeMap<Verdict, usize>,
    pub consensus_reached: bool,
    pub votes: Vec<AgentVote>,
    pub dissenters: Vec<String>,
}

/// Adjudicator for resolving agent disagreements
pub struct Adjudicator {
    // In practice, this would use an LLM or deterministic rules
}

impl Adjudicator {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn adjudicate(
        &self,
        candidate: &serde_json::Value,
        positions: Vec<AdjudicatorPosition>,
        code_graph_slice: Option<&serde_json::Value>,
        model: Option<&dyn ModelProvider>,
    ) -> Result<AdjudicationResult, ConsensusError> {
        // In practice, this would call an LLM or use deterministic rules
        // For now, return a stub
        Ok(AdjudicationResult {
            decision: "needs-review".to_string(),
            rationale: "Adjudication not implemented in this tranche".to_string(),
            side_taken: "none".to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjudicatorPosition {
    pub modality: String,
    pub origin: serde_json::Value,
    pub result: String,
    pub detail: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjudicationResult {
    pub decision: String,
    pub rationale: String,
    pub side_taken: String,
}

// Trait for model provider (to be implemented by the embedding application)
pub trait ModelProvider: Send + Sync {
    fn complete(&self, prompt: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, std::io::Error>> + Send>>;
}