//! mch-clock — the clock as a tool, never as volatile prompt context.
//!
//! Mined from OpenWorker (github.com/andrewyng/openworker, MIT, © 2024
//! Andrew Ng), `coworker/clock.py` (OPE-192). The invariant: the per-turn
//! context block is glued onto a message the provider has already cached, so
//! any value that changes by itself — the live clock — rewrites that message
//! every turn and discards the whole cached prefix. A time tool call costs a
//! few tokens only when time is actually needed. Logic re-derived, not copied.

use serde::Serialize;

/// Volatile values belong behind a tool; the cached context block must only
/// carry state the user (or the harness) explicitly changed. A harness passes
/// its per-turn context candidates here and keeps only the stable ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextCandidate {
    /// Moves on its own — must never ride the cached block (clock, counter).
    Volatile(&'static str),
    /// Changes only when something outside the loop changes it — safe.
    Stable(&'static str),
}

/// Returns true when the per-turn block may embed the candidate: every entry
/// must be stable. One volatile entry poisons the whole block, because the
/// block is rewritten as one message.
pub fn context_is_cache_safe(candidates: &[ContextCandidate]) -> bool {
    candidates.iter().all(|c| matches!(c, ContextCandidate::Stable(_)))
}

/// Which volatile values a harness should expose as tools instead.
pub fn volatile_to_tool(candidates: &[ContextCandidate]) -> Vec<&'static str> {
    candidates
        .iter()
        .filter_map(|c| match c {
            ContextCandidate::Volatile(name) => Some(*name),
            ContextCandidate::Stable(_) => None,
        })
        .collect()
}

/// What the `current_time` tool returns: local wall time with zone name, the
/// same instant in UTC, and the weekday — enough for deadlines ("how long
/// ago"), absolute wake times, and human-facing dates. Relative waits ("in
/// ten minutes") need none of it and should not cost a call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimeReport {
    /// RFC 3339 local time with offset, e.g. `2026-10-18T14:03:22+02:00`.
    pub local: String,
    /// IANA/OS zone abbreviation as reported by the host (`TZ` on unix).
    pub tzname: String,
    /// RFC 3339 UTC instant with the `Z` suffix.
    pub utc: String,
    /// English weekday of the local time (`Mon` … `Sun`).
    pub weekday: String,
}

pub fn current_time() -> TimeReport {
    let now = chrono::Local::now();
    let tzname = iana_or_abbrev(now);
    TimeReport {
        local: now.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        tzname,
        utc: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        weekday: now.format("%a").to_string(),
    }
}

#[cfg(unix)]
fn iana_or_abbrev(now: chrono::DateTime<chrono::Local>) -> String {
    match std::env::var("TZ") {
        Ok(tz) if !tz.is_empty() => tz,
        _ => now.format("%Z").to_string(),
    }
}

#[cfg(not(unix))]
fn iana_or_abbrev(now: chrono::DateTime<chrono::Local>) -> String {
    now.format("%Z").to_string()
}

/// The tool descriptor a harness can hand to the provider, matching what
/// `current_time` returns. Registered for every session, like OpenWorker's.
pub const CURRENT_TIME_TOOL_NAME: &str = "current_time";
pub const CURRENT_TIME_TOOL_SCHEMA: &str = r#"{"name":"current_time","description":"Get the current local and UTC time with timezone and weekday. Call when a deadline, elapsed time, or an absolute wake time is needed.","parameters":{"type":"object","properties":{},"required":[]}}"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_report_shape() {
        let t = current_time();
        assert!(t.local.contains('T'));
        assert!(t.utc.ends_with('Z'));
        assert!(!t.tzname.is_empty());
        assert_eq!(t.weekday.len(), 3);
    }

    #[test]
    fn stable_context_is_cache_safe() {
        let candidates = vec![
            ContextCandidate::Stable("mode notice"),
            ContextCandidate::Stable("root dirs"),
        ];
        assert!(context_is_cache_safe(&candidates));
    }

    #[test]
    fn volatile_context_is_rejected_wholesale() {
        // One self-moving value rewrites the whole glued-on message.
        let candidates = vec![
            ContextCandidate::Stable("mode notice"),
            ContextCandidate::Volatile("live clock"),
        ];
        assert!(!context_is_cache_safe(&candidates));
    }

    #[test]
    fn volatile_values_become_tools() {
        let candidates = vec![ContextCandidate::Volatile("live clock")];
        assert_eq!(volatile_to_tool(&candidates), vec!["live clock"]);
    }

    #[test]
    fn tool_descriptor_names_the_tool() {
        assert!(CURRENT_TIME_TOOL_SCHEMA.contains(CURRENT_TIME_TOOL_NAME));
    }
}
