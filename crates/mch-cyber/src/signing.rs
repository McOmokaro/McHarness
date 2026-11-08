//! Ed25519 Signing — Non-repudiation for security decisions
//!
//! Forged from Spotlight (CMUL8/spotlight) non-repudiation signing
//! Implements Ed25519 cryptographic attestation for scope decisions, evidence bundles,
//! and audit trails.

use crate::consensus::Verdict;
use base64::Engine as _;
use ed25519_dalek::{Signature, Signer as Ed25519Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Public signing key type (re-exported for downstream consumers).
pub use ed25519_dalek::SigningKey as Ed25519SigningKey;
/// Public verifying key type (re-exported for downstream consumers).
pub type VerifyKey = VerifyingKey;

/// A cryptographically signed action/decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedAction {
    /// The action that was signed
    pub action: String,
    /// The payload that was signed
    pub payload: serde_json::Value,
    /// Ed25519 signature of the payload
    pub signature: Vec<u8>,
    /// Public key of the signer
    pub public_key: Vec<u8>,
    /// Timestamp of signing (Unix ms)
    pub timestamp_ms: u64,
    /// Key fingerprint for key rotation tracking
    pub key_fingerprint: String,
}

/// The core action decision data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDecision {
    /// The target host or resource
    pub target: String,
    /// The scope verdict
    pub verdict: Verdict,
    /// The authority that made the decision
    pub authority_id: String,
    /// Optional consensus verdict if multi-agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consensus: Option<crate::consensus::ConsensusVerdict>,
    /// Optional warden detection result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warden_result: Option<crate::warden::DetectionResult>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

/// Signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    /// Base64-encoded Ed25519 private key (32 bytes raw)
    pub signing_key_b64: Option<String>,
    /// Authority ID for this signer
    pub authority_id: String,
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            signing_key_b64: None,
            authority_id: "mch-cyber".to_string(),
        }
    }
}

/// Action signer for scope decisions
pub struct ActionSigner {
    signing_key: SigningKey,
    authority_id: String,
}

impl ActionSigner {
    /// Create a new action signer with a generated key pair
    pub fn new(authority_id: String) -> Self {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let signing_key = SigningKey::from_bytes(&bytes);
        Self {
            signing_key,
            authority_id,
        }
    }

    /// Create a signer from an existing signing key
    pub fn from_key(signing_key: SigningKey, authority_id: String) -> Self {
        Self {
            signing_key,
            authority_id,
        }
    }

    /// Create from base64-encoded key
    pub fn from_b64(key_b64: &str, authority_id: String) -> Result<Self, SigningError> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(key_b64)
            .map_err(|e| SigningError::InvalidKey(e.to_string()))?;
        if raw.len() != 32 {
            return Err(SigningError::InvalidKey("Key must be 32 bytes".into()));
        }
        let signing_key = SigningKey::from_bytes(&raw.try_into().unwrap());
        Ok(Self {
            signing_key,
            authority_id,
        })
    }

    /// Get the verifying key for this signer
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Sign an action decision
    pub fn sign_decision(&self, decision: ActionDecision) -> Result<SignedAction, SigningError> {
        let payload = Self::decision_payload(&decision);
        let signature = self.signing_key.sign(&payload).to_bytes().to_vec();
        let public_key = self.signing_key.verifying_key().to_bytes().to_vec();
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(SignedAction {
            action: "scope-decision".to_string(),
            payload: serde_json::to_value(decision)?,
            signature,
            public_key,
            timestamp_ms,
            key_fingerprint: Self::fingerprint(&self.signing_key.verifying_key()),
        })
    }

    /// Verify a signed action decision
    pub fn verify_decision(signed: &SignedAction) -> Result<bool, SigningError> {
        let Ok(public_key) =
            VerifyingKey::from_bytes(&signed.public_key.as_slice().try_into().unwrap_or([0u8; 32]))
        else {
            return Ok(false);
        };
        let payload = Self::decision_payload(&serde_json::from_value(signed.payload.clone())?);
        Ok(public_key
            .verify(
                &payload,
                &Signature::from_bytes(&signed.signature.as_slice().try_into().unwrap_or([0u8; 64])),
            )
            .is_ok())
    }

    fn decision_payload(decision: &ActionDecision) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(decision.target.as_bytes());
        payload.push(decision.verdict as u8);
        payload.extend_from_slice(decision.authority_id.as_bytes());
        // Note: consensus and warden_result not included in payload for simplicity.
        // In production, would include serialized versions.
        for (k, v) in &decision.metadata {
            payload.extend_from_slice(k.as_bytes());
            payload.push(b'=');
            payload.extend_from_slice(v.as_bytes());
        }
        payload
    }

    /// Get the key fingerprint (first 16 hex chars of SHA-256 of public key)
    fn fingerprint(pubkey: &VerifyingKey) -> String {
        let raw = pubkey.to_bytes();
        let hash = Sha256::digest(raw);
        hex::encode(&hash[..16])
    }
}

/// Action verifier for scope decisions
pub struct ActionVerifier {
    verifying_key: VerifyingKey,
}

impl ActionVerifier {
    /// Create a verifier from a public key
    pub fn new(verifying_key: VerifyingKey) -> Self {
        Self { verifying_key }
    }

    /// Create from base64-encoded public key
    pub fn from_b64(key_b64: &str) -> Result<Self, SigningError> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(key_b64)
            .map_err(|e| SigningError::InvalidKey(e.to_string()))?;
        let verifying_key = VerifyingKey::from_bytes(&raw.try_into().unwrap_or([0u8; 32]))
            .map_err(|e| SigningError::InvalidKey(e.to_string()))?;
        Ok(Self { verifying_key })
    }

    /// Verify a signed action decision
    pub fn verify_decision(&self, signed: &SignedAction) -> Result<bool, SigningError> {
        let payload =
            ActionSigner::decision_payload(&serde_json::from_value(signed.payload.clone())?);
        Ok(self
            .verifying_key
            .verify(
                &payload,
                &Signature::from_bytes(&signed.signature.as_slice().try_into().unwrap_or([0u8; 64])),
            )
            .is_ok())
    }

    /// Verify strictly using verify_strict (rejects non-canonical signatures)
    pub fn verify_strict(&self, signed: &SignedAction) -> Result<bool, SigningError> {
        let payload =
            ActionSigner::decision_payload(&serde_json::from_value(signed.payload.clone())?);
        let sig =
            Signature::from_bytes(&signed.signature.as_slice().try_into().unwrap_or([0u8; 64]));
        Ok(self.verifying_key.verify_strict(&payload, &sig).is_ok())
    }
}

/// Scope attestation bundle for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeAttestation {
    /// The signed decision
    pub signed_decision: SignedAction,
    /// Chain of prior attestations (for tamper evidence)
    pub prior_attestations: Vec<SignedAction>,
    /// Merkle root of the attestation chain
    pub chain_root: Option<Vec<u8>>,
}

impl ScopeAttestation {
    /// Create a new attestation with the given decision and prior chain
    pub fn new(signed_decision: SignedAction, prior_attestations: Vec<SignedAction>) -> Self {
        let chain_root = Self::compute_chain_root(&prior_attestations);
        Self {
            signed_decision,
            prior_attestations,
            chain_root,
        }
    }

    /// Verify the entire attestation chain
    pub fn verify_chain(&self) -> Result<bool, SigningError> {
        if !ActionSigner::verify_decision(&self.signed_decision)? {
            return Ok(false);
        }
        for attestation in &self.prior_attestations {
            if !ActionSigner::verify_decision(attestation)? {
                return Ok(false);
            }
        }
        let computed_root = Self::compute_chain_root(&self.prior_attestations);
        if self.chain_root != computed_root {
            return Ok(false);
        }
        Ok(true)
    }

    fn compute_chain_root(attestations: &[SignedAction]) -> Option<Vec<u8>> {
        if attestations.is_empty() {
            return None;
        }
        let mut hasher = Sha256::new();
        for attestation in attestations {
            hasher.update(&attestation.signature);
        }
        Some(hasher.finalize().to_vec())
    }
}

/// Signing errors
#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("Ed25519 error: {0}")]
    Ed25519(String),
}

/// Convenience alias so `use mch_cyber::signing::Signer;` resolves to the
/// cryptographic signer wrapper rather than the ed25519 trait.
pub use ActionSigner as Signer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_roundtrip() {
        let signer = ActionSigner::new("test".into());
        let decision = ActionDecision {
            target: "example.com".into(),
            verdict: Verdict::InScope,
            authority_id: "test".into(),
            consensus: None,
            warden_result: None,
            metadata: BTreeMap::new(),
        };
        let signed = signer.sign_decision(decision).unwrap();
        assert!(ActionSigner::verify_decision(&signed).unwrap());
    }
}
