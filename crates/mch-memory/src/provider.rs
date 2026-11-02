//! The pluggable external-memory contract.
//!
//! Mined from Hermes Agent's `agent/memory_provider.py`. The built-in store is one provider
//! among possible others; at most one external provider is active. A provider gets the turn
//! lifecycle and, on every fan-out, already-redacted input — providers archive whatever they
//! are handed.

use serde::{Deserialize, Serialize};

/// What the last prefetch injected, for the recall indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallStatus {
    pub provider_label: String,
    pub count: usize,
}

/// A memory backend. The built-in store implements the same shape.
pub trait MemoryProvider {
    /// Short identifier, e.g. `builtin`.
    fn name(&self) -> &str;

    /// Configured, credentialed and ready? Gates activation. No network here.
    fn is_available(&self) -> bool {
        true
    }

    /// STATIC system-prompt text. Recalled context goes through `prefetch`, not here.
    fn system_prompt_block(&self) -> String {
        String::new()
    }

    /// Formatted recall context for the upcoming turn; empty when none. Must be fast.
    fn prefetch(&self, _query: &str) -> String {
        String::new()
    }

    /// What the most recent `prefetch` injected.
    fn recall_status(&self) -> Option<RecallStatus> {
        None
    }

    /// Persist a completed turn. Non-blocking in the reference; here it is a plain call.
    fn sync_turn(&self, _user: &str, _assistant: &str) {}

    /// Mirror a built-in memory write (`add` | `replace` | `remove`).
    fn on_memory_write(&self, _action: &str, _target: &str, _content: &str) {}

    /// End-of-session extraction; fires only at real session boundaries.
    fn on_session_end(&self, _messages: &[String]) {}

    /// Clean shutdown.
    fn shutdown(&self) {}
}
