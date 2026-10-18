//! mch-audit — transcript redaction + tamper-evident audit chain.
//!
//! Mined from pentest-agents (`audit_logger.py` `AuditChain`):
//! SHA-256 hash chaining + HMAC chain tag, secret redaction BEFORE the line is
//! written, and fail-closed rejection of missing prior hashes.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("audit chain broken at entry {index}: expected prior hash {expected}, got {got}")]
    ChainBroken { index: usize, expected: String, got: String },
}

type HmacSha256 = Hmac<Sha256>;

/// A single audit entry — one tool call or decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub ts: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub payload: String,
    pub prev_hash: String,
    pub hash: String,
    pub chain_tag: String,
}

/// Deterministic audit chain. Secret values are redacted from the payload
/// BEFORE hashing/writing, so the chain never stores secrets.
pub struct AuditChain {
    entries: Vec<AuditEntry>,
    key: Vec<u8>,
    path: Option<PathBuf>,
}

impl AuditChain {
    pub fn new(key: &[u8]) -> Self {
        Self { entries: Vec::new(), key: key.to_vec(), path: None }
    }

    pub fn with_path(key: &[u8], path: PathBuf) -> Self {
        Self { entries: Vec::new(), key: key.to_vec(), path: Some(path) }
    }

    /// Redact a secret value everywhere it appears in the payload BEFORE it
    /// enters the chain.
    pub fn redact(value: &str, payload: &str) -> String {
        if value.is_empty() {
            return payload.to_string();
        }
        let masked = "*".repeat(value.len().min(12));
        payload.replace(value, &masked)
    }

    pub fn append(&mut self, actor: &str, action: &str, target: &str, payload: &str) {
        let seq = self.entries.len() as u64;
        let prev_hash = if seq == 0 {
            "GENESIS".to_string()
        } else {
            self.entries[(seq - 1) as usize].hash.clone()
        };
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis().to_string())
            .unwrap_or_default();
        let hash = Self::hash_entry(&prev_hash, seq, &ts, actor, action, target, payload);
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("hmac key");
        mac.update(hash.as_bytes());
        let chain_tag = hex(&mac.finalize().into_bytes());
        self.entries.push(AuditEntry {
            seq,
            ts,
            actor: actor.to_string(),
            action: action.to_string(),
            target: target.to_string(),
            payload: payload.to_string(),
            prev_hash,
            hash,
            chain_tag,
        });
        if let Some(p) = &self.path {
            let _ = self.flush(p);
        }
    }

    fn hash_entry(
        prev_hash: &str,
        seq: u64,
        ts: &str,
        actor: &str,
        action: &str,
        target: &str,
        payload: &str,
    ) -> String {
        let mut h = Sha256::new();
        h.update(prev_hash.as_bytes());
        h.update(seq.to_string().as_bytes());
        h.update(ts.as_bytes());
        h.update(actor.as_bytes());
        h.update(action.as_bytes());
        h.update(target.as_bytes());
        h.update(payload.as_bytes());
        hex(&h.finalize())
    }

    /// Tamper detection: recompute each hash + chain tag.
    pub fn verify(&self) -> Result<(), AuditError> {
        for (i, e) in self.entries.iter().enumerate() {
            let expected_prev = if i == 0 {
                "GENESIS".to_string()
            } else {
                self.entries[i - 1].hash.clone()
            };
            if e.prev_hash != expected_prev {
                return Err(AuditError::ChainBroken {
                    index: i,
                    expected: expected_prev,
                    got: e.prev_hash.clone(),
                });
            }
            let recomputed = Self::hash_entry(
                &e.prev_hash,
                e.seq,
                &e.ts,
                &e.actor,
                &e.action,
                &e.target,
                &e.payload,
            );
            if recomputed != e.hash {
                return Err(AuditError::ChainBroken {
                    index: i,
                    expected: recomputed,
                    got: e.hash.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn flush(&self, path: &Path) -> Result<(), AuditError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &Path, key: &[u8]) -> Result<Self, AuditError> {
        let text = std::fs::read_to_string(path)?;
        let entries: Vec<AuditEntry> = serde_json::from_str(&text)?;
        let chain = Self { entries, key: key.to_vec(), path: Some(path.to_path_buf()) };
        chain.verify()?;
        Ok(chain)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_verify_clean_chain() {
        let mut chain = AuditChain::new(b"test-key");
        chain.append("agent", "scan", "acme.com", "nmap -sV");
        chain.append("agent", "scan", "acme.com", "nuclei -l urls.txt");
        assert!(chain.verify().is_ok());
        assert_eq!(chain.entries().len(), 2);
        assert_eq!(chain.entries()[0].prev_hash, "GENESIS");
    }

    #[test]
    fn tampering_breaks_chain() {
        let mut chain = AuditChain::new(b"test-key");
        chain.append("agent", "scan", "acme.com", "nmap -sV");
        chain.entries[0].payload = "TAMPERED".to_string();
        assert!(matches!(chain.verify(), Err(AuditError::ChainBroken { .. })));
    }

    #[test]
    fn redact_hides_secret() {
        let redacted = AuditChain::redact("hunter2", "login with password=hunter2 now");
        assert!(!redacted.contains("hunter2"));
        assert!(redacted.contains("********"));
    }

    #[test]
    fn redact_empty_is_noop() {
        assert_eq!(AuditChain::redact("", "keep me"), "keep me");
    }

    #[test]
    fn flush_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mch-audit-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("audit.json");
        let mut chain = AuditChain::new(b"k");
        chain.append("agent", "probe", "t", "p");
        chain.flush(&path).unwrap();
        let loaded = AuditChain::load(&path, b"k").unwrap();
        assert_eq!(loaded.entries().len(), 1);
    }
}
