//! mch-dsh-dynamic — dynamic package runtime with host/client halves, forged from
//! DeepSeek Harness `cordis-host-runner` and `cordis-client-runner`.
//!
//! A host half registers definitions and orchestrates a run round-trip; a client
//! half evaluates the browser half behind a whitelisting guard and mounts it
//! through the loader.

use std::collections::HashMap;

/// A request to run a dynamic package.
#[derive(Debug, Clone)]
pub struct RunRequest {
    pub definition_id: String,
    pub revision: u64,
}

/// The stage at which a run settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStage { HostHalf, SourceFetch, BrowserHalf, Resolved }

/// The outcome of a run request.
#[derive(Debug, Clone)]
pub struct RunResolution {
    pub accepted: bool,
    pub revision: Option<u64>,
    pub stage: RunStage,
}

/// Registry of dynamically loaded package definitions.
#[derive(Default)]
pub struct DynamicRegistry {
    packages: HashMap<String, u64>,
}

impl DynamicRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, id: impl Into<String>, revision: u64) {
        self.packages.insert(id.into(), revision);
    }

    pub fn remove(&mut self, id: &str) -> Option<u64> { self.packages.remove(id) }

    pub fn get(&self, id: &str) -> Option<u64> { self.packages.get(id).copied() }
}

/// Orchestrates a run round-trip (host half first, then the browser half).
#[derive(Default)]
pub struct HostOrchestrator;

impl HostOrchestrator {
    pub fn new() -> Self { Self }

    /// Resolve a run request against the currently loaded revision.
    pub fn resolve(&self, req: &RunRequest, current: Option<u64>) -> RunResolution {
        if current == Some(req.revision) {
            RunResolution { accepted: false, revision: current, stage: RunStage::Resolved }
        } else {
            RunResolution { accepted: true, revision: Some(req.revision), stage: RunStage::Resolved }
        }
    }
}

/// Host-side half of the dynamic runtime.
#[derive(Default)]
pub struct HostRunner {
    pub registry: DynamicRegistry,
    pub orchestrator: HostOrchestrator,
}

impl HostRunner {
    pub fn new() -> Self { Self::default() }
}

/// Guard facade whitelisting the services a loaded browser half may touch.
#[derive(Default)]
pub struct Guard {
    allowed: Vec<String>,
}

impl Guard {
    pub fn new() -> Self { Self::default() }

    pub fn allow(&mut self, name: impl Into<String>) { self.allowed.push(name.into()); }

    pub fn permits(&self, name: &str) -> bool { self.allowed.iter().any(|a| a == name) }
}

/// Evaluates a browser-half source into a plugin.
#[derive(Default)]
pub struct Evaluator;

impl Evaluator {
    pub fn new() -> Self { Self }
}

/// Mounts a loaded dynamic package through the loader.
#[derive(Default)]
pub struct Loader;

impl Loader {
    pub fn new() -> Self { Self }
}

/// Browser-side half of the dynamic runtime.
#[derive(Default)]
pub struct ClientRunner {
    pub guard: Guard,
    pub evaluator: Evaluator,
    pub loader: Loader,
}

impl ClientRunner {
    pub fn new() -> Self { Self::default() }
}

/// Re-export commonly used types.
pub mod prelude {
    pub use crate::{
        ClientRunner, DynamicRegistry, Guard, HostRunner, Loader, RunRequest, RunResolution,
        RunStage,
    };
}
