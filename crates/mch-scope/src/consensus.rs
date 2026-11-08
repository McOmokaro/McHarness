//! Consensus Kernel for agent disagreements (Spotlight architecture).
//!
//! Implements multi-agent consensus for scope decisions, preventing single-agent
// hijacking of scope authority. Uses deterministic voting with cryptographic
// audit trail.

use crate::{ScopeError, ScopeVerdict};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use rand_core::CryptoRngCore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zeroize::Zeroize;

/// Configuration for consensus behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Minimum number of agents required for consensus.
    pub min_agents: usize,
    /// Threshold ratio for consensus (e.g., 0.67 for 2/3 majority).
    pub threshold: f64,
    /// Whether to require unanimous consent for OutOfScope verdicts.
    pub require_unanimous_deny: bool,
    /// Maximum time to wait for votes (milliseconds).
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

/// A vote from a single agent in the consensus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVote {
    /// Unique agent identifier.
    pub agent_id: String,
    /// The verdict this agent votes for.
    pub verdict: ScopeVerdict,
    /// Cryptographic signature of the vote.
    pub signature: Vec<u8>,
    /// Public key of the voting agent.
    pub public_key: Vec<u8>,
    /// Timestamp of the vote (Unix ms).
    pub timestamp_ms: u64,
    /// Optional rationale.
    pub rationale: Option<String>,
}

impl AgentVote {
    /// Create a new signed vote.
    pub fn new(
        agent_id: String,
        verdict: ScopeVerdict,
        signing_key: &SigningKey,
        rationale: Option<String>,
    ) -> Self {
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

    /// Verify the vote signature.
    pub fn verify(&self) -> bool {
        let public_key_bytes: [u8; 32] = self.public_key.as_slice().try_into().unwrap_or([0u8; 32]);
        let Ok(public_key) = VerifyingKey::from_bytes(&public_key_bytes) else {
            return false;
        };
        let payload = Self::vote_payload(&self.agent_id, self.verdict, self.rationale.as_deref());
        let signature_bytes: [u8; 64] = self.signature.as_slice().try_into().unwrap_or([0u8; 64]);
        public_key.verify(&payload, &ed25519_dalek::Signature::from_bytes(&signature_bytes)).is_ok()
    }

    fn vote_payload(agent_id: &str, verdict: ScopeVerdict, rationale: Option<&str>) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(agent_id.as_bytes());
        payload.push(verdict as u8);
        if let Some(r) = rationale {
            payload.extend_from_slice(r.as_bytes());
        }
        payload
    }
}

/// Result of a consensus round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusVerdict {
    /// The agreed-upon verdict.
    pub verdict: ScopeVerdict,
    /// Number of agents that voted.
    pub participant_count: usize,
    /// Vote breakdown by verdict.
    pub vote_tally: BTreeMap<ScopeVerdict, usize>,
    /// Whether consensus was reached.
    pub consensus_reached: bool,
    /// All valid votes cast.
    pub votes: Vec<AgentVote>,
    /// Dissenting agents (if any).
    pub dissenters: Vec<String>,
}

/// Consensus Kernel: coordinates multi-agent agreement on scope decisions.
pub struct ConsensusKernel {
    config: ConsensusConfig,
    /// Registered agent verifying keys.
    agents: BTreeMap<String, VerifyingKey>,
}

impl ConsensusKernel {
    /// Create a new consensus kernel with registered agents.
    pub fn new(config: ConsensusConfig, agents: Vec<(String, VerifyingKey)>) -> Self {
        let agents = agents.into_iter().collect();
        Self { config, agents }
    }

    /// Register a new agent.
    pub fn register_agent(&mut self, agent_id: String, verifying_key: VerifyingKey) {
        self.agents.insert(agent_id, verifying_key);
    }

    /// Run consensus on a scope decision.
    ///
    /// Collects votes from all registered agents and determines consensus.
    /// Returns the consensus verdict or an error if quorum not met.
    pub fn decide(
        &self,
        host: &str,
        local_verdict: ScopeVerdict,
        votes: Vec<AgentVote>,
    ) -> Result<ConsensusVerdict, ScopeError> {
        // Verify all votes
        let mut valid_votes = Vec::new();
        for vote in votes {
            if !vote.verify() {
                return Err(ScopeError::ConsensusFailed(format!(
                    "Invalid signature from agent {}",
                    vote.agent_id
                )));
            }
            // Verify agent is registered
            if !self.agents.contains_key(&vote.agent_id) {
                return Err(ScopeError::ConsensusFailed(format!(
                    "Unregistered agent {}",
                    vote.agent_id
                )));
            }
            valid_votes.push(vote);
        }

        // Check quorum
        if valid_votes.len() < self.config.min_agents {
            return Err(ScopeError::ConsensusFailed(format!(
                "Quorum not met: {}/{} agents voted",
                valid_votes.len(),
                self.config.min_agents
            )));
        }

        // Tally votes
        let mut tally = BTreeMap::new();
        for vote in &valid_votes {
            *tally.entry(vote.verdict).or_insert(0) += 1;
        }

        let total = valid_votes.len() as f64;
        let in_scope_count = *tally.get(&ScopeVerdict::InScope).unwrap_or(&0) as f64;
        let out_scope_count = *tally.get(&ScopeVerdict::OutOfScope).unwrap_or(&0) as f64;

        // Determine consensus
        let consensus_reached;
        let verdict;
        let mut dissenters = Vec::new();

        if out_scope_count > 0.0 && self.config.require_unanimous_deny {
            // Deny-wins: any OutOfScope vote with unanimous deny requirement
            if out_scope_count / total >= 1.0 {
                consensus_reached = true;
                verdict = ScopeVerdict::OutOfScope;
            } else {
                consensus_reached = false;
                verdict = local_verdict; // Fall back to local
                for vote in &valid_votes {
                    if vote.verdict == ScopeVerdict::OutOfScope {
                        dissenters.push(vote.agent_id.clone());
                    }
                }
            }
        } else if in_scope_count / total >= self.config.threshold {
            consensus_reached = true;
            verdict = ScopeVerdict::InScope;
            for vote in &valid_votes {
                if vote.verdict != ScopeVerdict::InScope {
                    dissenters.push(vote.agent_id.clone());
                }
            }
        } else if out_scope_count / total >= self.config.threshold {
            consensus_reached = true;
            verdict = ScopeVerdict::OutOfScope;
            for vote in &valid_votes {
                if vote.verdict != ScopeVerdict::OutOfScope {
                    dissenters.push(vote.agent_id.clone());
                }
            }
        } else {
            consensus_reached = false;
            verdict = ScopeVerdict::Uncertain;
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

    /// Generate signing keys for a new agent (for testing/setup).
    pub fn generate_agent_keys<R: CryptoRngCore>(rng: &mut R) -> (SigningKey, VerifyingKey) {
        let mut bytes = [0u8; 32];
        rand::RngCore::fill_bytes(rng, &mut bytes);
        let signing_key = SigningKey::from_bytes(&bytes);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn consensus_reached_on_majority() {
        let mut rng = OsRng;
        let mut agents = Vec::new();
        let mut signing_keys = Vec::new();

        for i in 0..5 {
            let (sk, vk) = ConsensusKernel::generate_agent_keys(&mut rng);
            agents.push((format!("agent{}", i), vk));
            signing_keys.push(sk);
        }

        let config = ConsensusConfig {
            min_agents: 3,
            threshold: 0.6,
            require_unanimous_deny: false,
            timeout_ms: 5000,
        };

        let kernel = ConsensusKernel::new(config, agents);

        // 3 vote InScope, 2 vote Uncertain -> consensus InScope
        let mut votes = Vec::new();
        for i in 0..3 {
            votes.push(AgentVote::new(
                format!("agent{}", i),
                ScopeVerdict::InScope,
                &signing_keys[i],
                Some("in scope".into()),
            ));
        }
        for i in 3..5 {
            votes.push(AgentVote::new(
                format!("agent{}", i),
                ScopeVerdict::Uncertain,
                &signing_keys[i],
                Some("unsure".into()),
            ));
        }

        let result = kernel.decide("example.com", ScopeVerdict::Uncertain, votes).unwrap();
        assert!(result.consensus_reached);
        assert_eq!(result.verdict, ScopeVerdict::InScope);
    }

    #[test]
    fn deny_wins_with_unanimous_requirement() {
        let mut rng = OsRng;
        let mut agents = Vec::new();
        let mut signing_keys = Vec::new();

        for i in 0..4 {
            let (sk, vk) = ConsensusKernel::generate_agent_keys(&mut rng);
            agents.push((format!("agent{}", i), vk));
            signing_keys.push(sk);
        }

        let config = ConsensusConfig {
            min_agents: 3,
            threshold: 0.67,
            require_unanimous_deny: true,
            timeout_ms: 5000,
        };

        let kernel = ConsensusKernel::new(config, agents);

        // 3 vote InScope, 1 votes OutOfScope -> no consensus (deny not unanimous)
        let mut votes = Vec::new();
        for i in 0..3 {
            votes.push(AgentVote::new(
                format!("agent{}", i),
                ScopeVerdict::InScope,
                &signing_keys[i],
                Some("in scope".into()),
            ));
        }
        votes.push(AgentVote::new(
            "agent3".into(),
            ScopeVerdict::OutOfScope,
            &signing_keys[3],
            Some("suspicious".into()),
        ));

        let result = kernel.decide("malicious.com", ScopeVerdict::InScope, votes).unwrap();
        assert!(!result.consensus_reached);
        assert_eq!(result.dissenters.len(), 1);
        assert_eq!(result.dissenters[0], "agent3");
    }

    #[test]
    fn vote_verification_fails_on_tampering() {
        let mut rng = OsRng;
        let (sk, vk) = ConsensusKernel::generate_agent_keys(&mut rng);

        let mut vote = AgentVote::new(
            "agent1".into(),
            ScopeVerdict::InScope,
            &sk,
            Some("test".into()),
        );

        // Tamper with verdict
        vote.verdict = ScopeVerdict::OutOfScope;
        assert!(!vote.verify());
    }
}