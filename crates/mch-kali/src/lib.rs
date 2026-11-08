//! mch-kali — The Omarchy lazy-loading Kali Linux operator.
//!
//! McHarness does not bundle Kali. Instead this crate ships *stubs* for the
//! core Kali tools (nmap, ffuf, sqlmap, nuclei, subfinder, whatweb, …) and a
//! [`LazyLoader`] that installs a tool on demand the first time the agent
//! asks for it — then unloads it to stay featherweight.
//!
//! On top of the wrappers sits a [`TacticalPlanner`]: given a target it
//! fingerprints the tech stack and emits a scored, ordered plan of the exact
//! tool + flags to use (e.g. WordPress → wpscan + ffuf with a WP wordlist).
//!
//! Every tool run can be [`attest::attest_output`]'d with an Ed25519 key so a
//! dump (e.g. a sqlmap result) can be cryptographically proven authentic.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │                         mch-kali                              │
//! ├───────────────┬───────────────────┬──────────────────────────┤
//! │  Tool Layer   │  Lazy Loader      │  Tactical Engine          │
//! ├───────────────┼───────────────────┼──────────────────────────┤
//! │ KaliTool      │ apt-get on demand │ fingerprint (whatweb/nmap)│
//! │ ToolStub      │ docker single-tool │ score candidate tools     │
//! │ wrappers      │ unload after run  │ ordered TacticalPlan      │
//! │ JSON parsers  │ dry-run mode      │ WordPress→wpscan pivot    │
//! └───────────────┴───────────────────┴──────────────────────────┘
//!                        │
//!                  Ed25519 attestation (mch-cyber)
//! ```

pub mod attest;
pub mod bridge;
pub mod lazy;
pub mod output;
pub mod tactical;
pub mod tools;

pub use attest::{attest_output, output_digest, verify_attested, AttestedOutput};
pub use bridge::{deployments_for, execute_plan, AgentDeployment};
pub use lazy::{LazyLoadConfig, LazyLoader, LoadStrategy};
pub use output::ToolOutput;
pub use tactical::{
    FingerprintAnalyzer, PlannedAction, TacticalPlan, TacticalPlanner, TacticalPlannerConfig,
    TargetFingerprint, TechStack,
};
pub use tools::{
    run_raw, Ffuf, KaliTool, Nmap, Nuclei, Sqlmap, Subfinder, ToolCategory, ToolRegistry, ToolStub,
    Whatweb,
};
