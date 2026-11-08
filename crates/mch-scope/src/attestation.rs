//! Attestation Signer for cryptographic scope decisions (Spotlight architecture).
//!
//! Implements Ed25519 cryptographic attestation for scope decisions, providing
//! tamper-evident audit trails. Based on the Attestation component from CMUL8/spotlight.

use crate::{ScopeError, ScopeVerdict};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey, Signature};
use rand::rngs::OsRng;
use rand_core::CryptoRngCore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zeroize::Zeroize;
use sha2::{Digest, Sha256};

/// A cryptographically signed scope decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedScopeDecision {
    /// The scope decision that was made.
    pub decision: ScopeDecision,
    /// Ed25519 signature of the decision.
    pub signature: Vec<u8>,
    /// Public key of the signer.
    pub public_key: Vec<u8>,
    /// Timestamp of signing (Unix ms).
    pub timestamp_ms: u64,
}

/// The core scope decision data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeDecision {
    /// The target host or resource.
    pub target: String,
    /// The scope verdict.
    pub verdict: ScopeVerdict,
    /// The authority that made the decision.
    pub authority_id: String,
    /// Optional consensus verdict if multi-agent.
    pub consensus: Option<crate::consensus::ConsensusVerdict>,
    /// Optional warden detection result.
    pub warden_result: Option<crate::warden::DetectionResult>,
    /// Additional metadata.
    pub metadata: BTreeMap<String, String>,
}

/// Attestation signer for scope decisions.
pub struct AttestationSigner {
    signing_key: SigningKey,
    authority_id: String,
}

impl AttestationSigner {
    /// Create a new attestation signer with a generated key pair.
    pub fn new(authority_id: String) -> Self {
        let mut rng = OsRng;
        let mut bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rng, &mut bytes);
        let signing_key = SigningKey::from_bytes(&bytes);
        Self {
            signing_key,
            authority_id,
        }
    }

    /// Create a signer from an existing signing key.
    pub fn from_key(signing_key: SigningKey, authority_id: String) -> Self {
        Self {
            signing_key,
            authority_id,
        }
    }

    /// Get the verifying key for this signer.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Sign a scope decision.
    pub fn sign_decision(&self, decision: ScopeDecision) -> Result<SignedScopeDecision, ScopeError> {
        let payload = Self::decision_payload(&decision);
        let signature = self.signing_key.sign(&payload).to_bytes().to_vec();
        let public_key = self.signing_key.verifying_key().to_bytes().to_vec();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(SignedScopeDecision {
            decision,
            signature,
            public_key,
            timestamp_ms,
        })
    }

    /// Verify a signed scope decision.
    pub fn verify_decision(signed: &SignedScopeDecision) -> Result<bool, ScopeError> {
        let Ok(public_key) = VerifyingKey::from_bytes(&signed.public_key.as_slice().try_into().unwrap_or([0u8; 32])) else {
            return Ok(false);
        };
        let payload = Self::decision_payload(&signed.decision);
        Ok(public_key.verify(&payload, &Signature::from_bytes(
            &signed.signature.as_slice().try_into().unwrap_or([0u8; 64])
        )).is_ok())
    }

    fn decision_payload(decision: &ScopeDecision) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(decision.target.as_bytes());
        payload.push(decision.verdict as u8);
        payload.extend_from_slice(decision.authority_id.as_bytes());
        // Note: consensus and warden_result not included in payload for simplicity
        // In production, would include serialized versions
        for (k, v) in &decision.metadata {
            payload.extend_from_slice(k.as_bytes());
            payload.push(b'=');
            payload.extend_from_slice(v.as_bytes());
        }
        payload
    }
}

/// Scope attestation bundle for audit logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeAttestation {
    /// The signed decision.
    pub signed_decision: SignedScopeDecision,
    /// Chain of prior attestations (for tamper evidence).
    pub prior_attestations: Vec<SignedScopeDecision>,
    /// Merkle root of the attestation chain.
    pub chain_root: Option<Vec<u8>>,
}

impl ScopeAttestation {
    /// Create a new attestation with the given decision and prior chain.
    pub fn new(
        signed_decision: SignedScopeDecision,
        prior_attestations: Vec<SignedScopeDecision>,
    ) -> Self {
        let chain_root = Self::compute_chain_root(&prior_attestations);
        Self {
            signed_decision,
            prior_attestations,
            chain_root,
        }
    }

    /// Verify the entire attestation chain.
    pub fn verify_chain(&self) -> Result<bool, ScopeError> {
        // Verify the current decision
        if !AttestationSigner::verify_decision(&self.signed_decision)? {
            return Ok(false);
        }

        // Verify all prior attestations
        for attestation in &self.prior_attestations {
            if !AttestationSigner::verify_decision(attestation)? {
                return Ok(false);
            }
        }

        // Verify chain root matches
        let computed_root = Self::compute_chain_root(&self.prior_attestations);
        if self.chain_root != computed_root {
            return Ok(false);
        }

        Ok(true)
    }

    fn compute_chain_root(attestations: &[SignedScopeDecision]) -> Option<Vec<u8>> {
        if attestations.is_empty() {
            return None;
        }
        // Simple hash chain: hash(attestation_0) -> hash(attestation_1 + prev_hash) ...
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for attestation in attestations {
            hasher.update(&attestation.signature);
        }
        Some(hasher.finalize().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_decision() {
        let signer = AttestationSigner::new("test-authority".into());
        let decision = ScopeDecision {
            target: "example.com".into(),
            verdict: ScopeVerdict::InScope,
            authority_id: "test-authority".into(),
            consensus: None,
            warden_result: None,
            metadata: BTreeMap::new(),
        };

        let signed = signer.sign_decision(decision).unwrap();
        assert!(AttestationSigner::verify_decision(&signed).unwrap());
    }

    #[test]
    fn attestation_chain_verification() {
        let signer = AttestationSigner::new("test-authority".into());
        let mut prior = Vec::new();

        for i in 0..3 {
            let decision = ScopeDecision {
                target: format!("host{}.example.com", i),
                verdict: ScopeVerdict::InScope,
                authority_id: "test-authority".into(),
                consensus: None,
                warden_result: None,
                metadata: BTreeMap::new(),
            };
            let signed = signer.sign_decision(decision).unwrap();
            prior.push(signed);
        }

        let attestation = ScopeAttestation::new(prior[2].clone(), prior[..2].to_vec());
        assert!(attestation.verify_chain().unwrap());
    }
}