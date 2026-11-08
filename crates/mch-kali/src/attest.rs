//! Cryptographic attestation of Kali tool output.
//!
//! Proves that a dump (e.g. a sqlmap result) is authentic and untampered by
//! signing a canonical hash of the `ToolOutput` with an Ed25519 key via
//! `mch_cyber::signing` (the Spotlight-derived non-repudiation layer).

use crate::output::ToolOutput;
use mch_cyber::signing::{ActionSigner, SignedAction, SigningError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A tool output bundled with a cryptographic attestation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestedOutput {
    /// The signed tool output.
    pub output: ToolOutput,
    /// SHA-256 (hex) of the canonical output payload.
    pub output_hash: String,
    /// Ed25519 signature over the output hash + metadata.
    pub attestation: SignedAction,
}

/// Canonicalize a `ToolOutput` into a stable byte string for hashing.
fn canonical_bytes(output: &ToolOutput) -> Vec<u8> {
    // Sort keys implicitly by using serde_json::to_string on a fixed struct;
    // we include the fields that matter for authenticity.
    let payload = serde_json::json!({
        "tool": output.tool,
        "target": output.target,
        "exit_code": output.exit_code,
        "raw": output.raw,
        "parsed": output.parsed,
        "timestamp": output.timestamp,
    });
    // serde_json object ordering is insertion-stable for this literal.
    serde_json::to_string(&payload)
        .unwrap_or_default()
        .into_bytes()
}

/// Compute the canonical SHA-256 hex digest of a tool output.
pub fn output_digest(output: &ToolOutput) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_bytes(output));
    hex_encode(&hasher.finalize())
}

/// Attest a tool output with a signer.
pub fn attest_output(
    signer: &ActionSigner,
    output: &ToolOutput,
) -> Result<AttestedOutput, SigningError> {
    let digest = output_digest(output);
    let decision = mch_cyber::signing::ActionDecision {
        target: output.target.clone(),
        verdict: mch_cyber::consensus::Verdict::InScope,
        authority_id: "mch-kali".to_string(),
        consensus: None,
        warden_result: None,
        metadata: {
            let mut m = std::collections::BTreeMap::new();
            m.insert("tool".to_string(), output.tool.clone());
            m.insert("output_hash".to_string(), digest.clone());
            m.insert("exit_code".to_string(), output.exit_code.to_string());
            m
        },
    };
    let attestation = signer.sign_decision(decision)?;
    Ok(AttestedOutput {
        output: output.clone(),
        output_hash: digest,
        attestation,
    })
}

/// Verify an attested output: the embedded digest must match the recomputed
/// digest AND the Ed25519 signature must verify.
pub fn verify_attested(attested: &AttestedOutput) -> bool {
    // 1. Content integrity.
    let recomputed = output_digest(&attested.output);
    if recomputed != attested.output_hash {
        return false;
    }
    // 2. Signature validity.
    ActionSigner::verify_decision(&attested.attestation).unwrap_or(false)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_output() -> ToolOutput {
        let mut o = ToolOutput::new("sqlmap", "http://target/x?id=1", 0);
        o.raw = "[INFO] GET parameter 'id' is vulnerable".into();
        o.parsed = serde_json::json!({ "injectable": true, "dbms": "MySQL" });
        o
    }

    #[test]
    fn attest_and_verify_roundtrip() {
        let signer = ActionSigner::new("mch-kali-test".into());
        let attested = attest_output(&signer, &sample_output()).unwrap();
        assert!(verify_attested(&attested));
    }

    #[test]
    fn tampered_output_fails_verification() {
        let signer = ActionSigner::new("mch-kali-test".into());
        let mut attested = attest_output(&signer, &sample_output()).unwrap();
        // Tamper with the parsed payload after signing.
        attested.output.parsed = serde_json::json!({ "injectable": false });
        assert!(!verify_attested(&attested));
    }

    #[test]
    fn digest_is_stable() {
        let o = sample_output();
        assert_eq!(output_digest(&o), output_digest(&o));
    }
}
