//! mch-compaction — run-state compaction with handoff briefs.
//!
//! Mined from pentest-agents (`engine.py` `Compactor` / `summarize.py`):
//! runs compress to a handoff brief for the next agent; briefs are LLM-able but
//! the compaction trigger + mandatory fields are deterministic code.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompactionError {
    #[error("cannot compact: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunState {
    pub target: String,
    pub phase: String,
    pub findings: Vec<String>,
    pub exhausted: Vec<String>,
    pub open_threads: Vec<String>,
    pub tokens_used: u64,
    pub turns: u64,
}

impl Default for RunState {
    fn default() -> Self {
        Self {
            target: String::new(),
            phase: "recon".into(),
            findings: Vec::new(),
            exhausted: Vec::new(),
            open_threads: Vec::new(),
            tokens_used: 0,
            turns: 0,
        }
    }
}

/// The handoff brief — what the next agent receives instead of raw history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandoffBrief {
    pub target: String,
    pub phase: String,
    pub findings: Vec<String>,
    pub exhausted: Vec<String>,
    pub open_threads: Vec<String>,
    pub next_actions: Vec<String>,
    pub compacted_at: u64,
    pub source_turns: u64,
}

impl fmt::Display for HandoffBrief {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "TARGET: {}", self.target)?;
        writeln!(f, "PHASE: {}", self.phase)?;
        writeln!(f, "FINDINGS:")?;
        for x in &self.findings {
            writeln!(f, "  - {x}")?;
        }
        writeln!(f, "EXHAUSTED — do not retry:")?;
        for x in &self.exhausted {
            writeln!(f, "  - {x}")?;
        }
        writeln!(f, "OPEN THREADS:")?;
        for x in &self.open_threads {
            writeln!(f, "  - {x}")?;
        }
        writeln!(f, "NEXT ACTIONS:")?;
        for x in &self.next_actions {
            writeln!(f, "  - {x}")?;
        }
        Ok(())
    }
}

/// Compaction policy: deterministic trigger thresholds.
pub struct Compactor {
    token_threshold: u64,
    turn_threshold: u64,
}

impl Default for Compactor {
    fn default() -> Self {
        Self { token_threshold: 100_000, turn_threshold: 40 }
    }
}

impl Compactor {
    pub fn new(token_threshold: u64, turn_threshold: u64) -> Self {
        Self { token_threshold, turn_threshold }
    }

    /// Should this run be compacted now?
    pub fn should_compact(&self, state: &RunState) -> bool {
        state.tokens_used >= self.token_threshold || state.turns >= self.turn_threshold
    }

    /// Compress a run into a handoff brief. Open threads become next actions.
    pub fn compact(&self, state: &RunState) -> Result<HandoffBrief, CompactionError> {
        if state.target.is_empty() {
            return Err(CompactionError::Invalid("run has no target".into()));
        }
        let next_actions = state
            .open_threads
            .iter()
            .map(|t| format!("resume: {t}"))
            .collect();
        Ok(HandoffBrief {
            target: state.target.clone(),
            phase: state.phase.clone(),
            findings: state.findings.clone(),
            exhausted: state.exhausted.clone(),
            open_threads: state.open_threads.clone(),
            next_actions,
            compacted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            source_turns: state.turns,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(turns: u64, tokens: u64) -> RunState {
        RunState {
            target: "acme.com".into(),
            phase: "exploit".into(),
            findings: vec!["xss in /search".into()],
            exhausted: vec!["sqli/oracle-blind".into()],
            open_threads: vec!["test upload endpoint".into()],
            tokens_used: tokens,
            turns,
        }
    }

    #[test]
    fn no_compaction_below_thresholds() {
        let c = Compactor::default();
        assert!(!c.should_compact(&state(1, 1000)));
    }

    #[test]
    fn compacts_on_tokens() {
        let c = Compactor::default();
        assert!(c.should_compact(&state(1, 200_000)));
    }

    #[test]
    fn compacts_on_turns() {
        let c = Compactor::default();
        assert!(c.should_compact(&state(50, 100)));
    }

    #[test]
    fn brief_has_mandatory_fields() {
        let c = Compactor::default();
        let brief = c.compact(&state(3, 500)).unwrap();
        assert_eq!(brief.target, "acme.com");
        assert_eq!(brief.findings.len(), 1);
        assert_eq!(brief.exhausted.len(), 1);
        assert!(!brief.next_actions.is_empty());
        assert!(brief.next_actions[0].starts_with("resume:"));
    }

    #[test]
    fn brief_display_is_agent_readable() {
        let c = Compactor::default();
        let brief = c.compact(&state(3, 500)).unwrap();
        let text = brief.to_string();
        assert!(text.contains("TARGET: acme.com"));
        assert!(text.contains("EXHAUSTED — do not retry"));
    }

    #[test]
    fn compact_rejects_targetless_run() {
        let c = Compactor::default();
        let mut s = state(3, 500);
        s.target = String::new();
        assert!(c.compact(&s).is_err());
    }

    #[test]
    fn brief_serializes() {
        let c = Compactor::default();
        let brief = c.compact(&state(3, 500)).unwrap();
        let json = serde_json::to_string(&brief).unwrap();
        let back: HandoffBrief = serde_json::from_str(&json).unwrap();
        assert_eq!(back, brief);
    }
}
