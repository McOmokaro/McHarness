//! Evidence Bundles — Chain of custody for security findings
//!
//! Forged from Spotlight evidence bundles and CTF-agent findings sharing

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Evidence bundle — immutable record of a security finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceBundle {
    /// Unique bundle ID
    pub bundle_id: String,
    /// Finding this evidence supports
    pub finding_id: String,
    /// Individual evidence items
    pub items: Vec<EvidenceItem>,
    /// Chain of custody log
    pub chain_of_custody: ChainOfCustody,
    /// Cryptographic attestation
    pub attestation: Option<Attestation>,
    /// Creation timestamp
    pub created_at: String,
    /// Bundle version
    pub version: u32,
}

/// Individual evidence item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    /// Unique item ID
    pub item_id: String,
    /// Type of evidence
    pub evidence_type: EvidenceType,
    /// Source of evidence
    pub source: String,
    /// Raw evidence data
    pub data: serde_json::Value,
    /// SHA-256 hash of data for integrity
    pub data_hash: String,
    /// Collection timestamp
    pub collected_at: String,
    /// Collector identity
    pub collector: String,
}

/// Types of evidence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    /// Static analysis finding (Semgrep, etc.)
    StaticAnalysis,
    /// Dynamic reproduction (exploit execution)
    DynamicReproduction,
    /// Network capture (PCAP, HAR)
    NetworkCapture,
    /// Screenshot/visual evidence
    Visual,
    /// Log entry
    Log,
    /// Configuration file
    Configuration,
    /// Source code snippet
    SourceCode,
    /// Binary artifact
    Binary,
    /// Memory dump
    MemoryDump,
    /// Other
    Other,
}

/// Chain of custody for evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainOfCustody {
    /// Sequential custody records
    pub entries: Vec<CustodyEntry>,
    /// Merkle root of the chain
    pub merkle_root: Option<String>,
}

/// Single custody entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustodyEntry {
    /// Entry sequence number
    pub sequence: u64,
    /// Actor who took custody
    pub actor: String,
    /// Actor type (human, agent, system)
    pub actor_type: String,
    /// Action taken
    pub action: CustodyAction,
    /// Timestamp
    pub timestamp: String,
    /// Hash of evidence at this point
    pub evidence_hash: String,
    /// Signature of this entry
    pub signature: Option<String>,
}

/// Custody actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustodyAction {
    Created,
    Transferred,
    Analyzed,
    Verified,
    Signed,
    Archived,
    TransferredOut,
}

/// Cryptographic attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Attestation ID
    pub attestation_id: String,
    /// Attestor identity
    pub attester: String,
    /// Attester type
    pub attester_type: String,
    /// What is being attested
    pub subject: String,
    /// Attestation payload
    pub payload: serde_json::Value,
    /// Ed25519 signature
    pub signature: String,
    /// Public key of attester
    pub public_key: String,
    /// Timestamp
    pub timestamp: String,
    /// Key fingerprint
    pub key_fingerprint: String,
}

/// Evidence bundle builder
pub struct EvidenceBundleBuilder {
    finding_id: String,
    items: Vec<EvidenceItem>,
    chain_of_custody: ChainOfCustody,
    attestation: Option<Attestation>,
}

impl EvidenceBundleBuilder {
    pub fn new(finding_id: String) -> Self {
        Self {
            finding_id,
            items: Vec::new(),
            chain_of_custody: ChainOfCustody {
                entries: Vec::new(),
                merkle_root: None,
            },
            attestation: None,
        }
    }

    pub fn add_item(mut self, item: EvidenceItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn add_custody_entry(mut self, entry: CustodyEntry) -> Self {
        self.chain_of_custody.entries.push(entry);
        self
    }

    pub fn with_attestation(mut self, attestation: Attestation) -> Self {
        self.attestation = Some(attestation);
        self
    }

    pub fn build(self) -> EvidenceBundle {
        let now = chrono::Utc::now().to_rfc3339();
        let bundle_id = format!("evidence-{}", uuid::Uuid::new_v4());
        
        // Compute merkle root
        let merkle_root = Self::compute_merkle_root(&self.chain_of_custody.entries);
        
        EvidenceBundle {
            bundle_id,
            finding_id: self.finding_id,
            items: self.items,
            chain_of_custody: ChainOfCustody {
                entries: self.chain_of_custody.entries,
                merkle_root,
            },
            attestation: self.attestation,
            created_at: chrono::Utc::now().to_rfc3339(),
            version: 1,
        }
    }

    fn compute_merkle_root(entries: &[CustodyEntry]) -> Option<String> {
        if entries.is_empty() {
            return None;
        }
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for entry in entries {
            let entry_json = serde_json::to_string(entry).unwrap_or_default();
            hasher.update(entry_json.as_bytes());
        }
        Some(hex::encode(hasher.finalize()))
    }
}

impl EvidenceBundle {
    /// Verify the integrity of the evidence bundle
    pub fn verify(&self) -> bool {
        // Verify each item's hash
        for item in &self.items {
            let item_json = serde_json::to_string(&item.data).unwrap_or_default();
            let hash = sha2::Sha256::digest(item_json.as_bytes());
            if hex::encode(hash) != item.data_hash {
                return false;
            }
        }

        // Verify chain of custody integrity
        if !self.chain_of_custody.verify() {
            return false;
        }

        // Verify attestation if present
        if let Some(attestation) = &self.attestation {
            if !attestation.verify() {
                return false;
            }
        }

        true
    }
}

impl ChainOfCustody {
    /// Verify chain integrity
    pub fn verify(&self) -> bool {
        for i in 1..self.entries.len() {
            let prev = &self.entries[i - 1];
            let curr = &self.entries[i];
            // In a real implementation, would verify signatures and hash chain
            if curr.sequence != prev.sequence + 1 {
                return false;
            }
        }
        true
    }

    pub fn add_entry(&mut self, entry: CustodyEntry) {
        self.entries.push(entry);
    }
}

impl Attestation {
    /// Verify the attestation signature
    pub fn verify(&self) -> bool {
        use base64::engine::general_purpose;
        use base64::Engine as _;
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        let Ok(pk_bytes) = general_purpose::STANDARD.decode(&self.public_key) else {
            return false;
        };
        let Ok(pk): Result<[u8; 32], _> = pk_bytes.as_slice().try_into() else {
            return false;
        };
        let Ok(public_key) = VerifyingKey::from_bytes(&pk) else {
            return false;
        };

        let payload = format!(
            "{}|{}|{}|{}",
            self.subject, self.attester, self.timestamp, self.payload
        );

        let Ok(sig_bytes) = general_purpose::STANDARD.decode(&self.signature) else {
            return false;
        };
        let Ok(sig): Result<[u8; 64], _> = sig_bytes.as_slice().try_into() else {
            return false;
        };
        let signature = Signature::from_bytes(&sig);

        public_key.verify(payload.as_bytes(), &signature).is_ok()
    }
}