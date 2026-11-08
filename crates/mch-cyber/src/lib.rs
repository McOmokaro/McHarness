//! mch-cyber — Unified Cybersecurity Operations
//!
//! Forged from:
//! - **Spotlight (CMUL8/spotlight)**: Consensus Kernel, Warden (prompt injection/backdoor detection), Ed25519 non-repudiation signing
//! - **BugBountyFramework (h4ckologic/bughunter-ai)**: State-machine hunt orchestration, credential vault, auth automation, Burp MCP bridge, parallel agent execution
//! - **CTF-agent (verialabs/ctf-agent)**: Parallel racing architecture (swarm), multi-model coordinators (Claude, Codex, Pydantic AI), quota fallback, message bus
//!
//! The Kali toolkit lives in the companion `mch-kali` crate and is wireable
//! via the `kali` feature.

pub mod consensus;
pub mod evidence;
pub mod hunt;
pub mod sandbox;
pub mod signing;
pub mod swarm;
pub mod warden;

/// Shared `ModelProvider` trait — lets the Swarm and Adjudicator swap models dynamically.
pub use consensus::ModelProvider;

// ── consensus ────────────────────────────────────────────────────────
pub use consensus::{
    AdjudicationResult, Adjudicator, AdjudicatorPosition, AgentVote, ConsensusConfig,
    ConsensusError, ConsensusKernel, ConsensusVerdict, EvidenceItem, EvidenceModality, Tier,
    TierDecision, Verdict,
};

// ── warden ───────────────────────────────────────────────────────────
pub use warden::{
    BackdoorMatch, DetectionResult, InjectionMatch, PatternMatch, RiskLevel, ThreatClassification,
    WardenConfig, WardenConfigBuilder, WardenError, WardenService,
};

// ── signing ──────────────────────────────────────────────────────────
pub use signing::{
    ActionDecision, ActionSigner, ActionVerifier, Ed25519SigningKey as SigningKey, ScopeAttestation,
    SignedAction, Signer, SigningConfig, SigningError, VerifyKey,
};

// ── hunt ─────────────────────────────────────────────────────────────
pub use hunt::{
    AttackChain, AuthManager, BrowserHarness, BurpBridge, CredentialVault, Finding,
    FindingSeverity, HuntConfig, HuntOrchestrator, HuntPhase, HuntSession, HuntState, PhaseState,
    PhaseStatus, ReportGenerator, ScopeSet, SeverityProfile,
};

// ── swarm ────────────────────────────────────────────────────────────
pub use swarm::{
    ChallengeSwarm, Coordinator, MessageBus, ModelSpec, Provider, SolverResult, SolverSpec,
    SolverStatus, SwarmConfig, SwarmStatus,
};

// ── evidence ─────────────────────────────────────────────────────────
pub use evidence::{
    Attestation, ChainOfCustody, CustodyAction, CustodyEntry, EvidenceBundle, EvidenceBundleBuilder,
    EvidenceType,
};
/// Evidence item used inside evidence bundles (aliased to avoid clash with
/// the consensus `EvidenceItem`).
pub use evidence::EvidenceItem as BundleEvidenceItem;

// ── sandbox ──────────────────────────────────────────────────────────
pub use sandbox::{
    CapabilityRole, CapabilityToken, DockerSandbox, ExecutionResult, Sandbox, SandboxConfig,
    SandboxFactory, SandboxType,
};

/// Re-export key types for convenience
pub mod prelude {
    pub use crate::consensus::{ConsensusKernel, ConsensusConfig, ConsensusVerdict, TierDecision, Tier};
    pub use crate::warden::{WardenService, WardenConfig, DetectionResult, ThreatClassification, RiskLevel};
    pub use crate::signing::{ActionSigner as Signer, VerifyKey, SignedAction, ActionVerifier};
    pub use crate::hunt::{HuntOrchestrator, HuntConfig, HuntPhase, HuntState, HuntSession, SeverityProfile};
    pub use crate::swarm::{ChallengeSwarm, SwarmConfig, SolverResult, Provider, Coordinator};
    pub use crate::sandbox::{Sandbox, SandboxConfig, ExecutionResult, CapabilityToken};
}
