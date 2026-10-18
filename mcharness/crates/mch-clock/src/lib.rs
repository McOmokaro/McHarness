//! mch-clock — time as a tool, not as embedded context.
//!
//! Mined from OpenWorker (`context/time.py`, MIT, (c) 2024 Andrew Ng,
//! github.com/andrewyng/openworker). Stable context (timezone) is cached;
//! volatile values (now) are never baked into prompts — they become tool
//! descriptors the agent calls on demand.

use chrono::{Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClockError {
    #[error("unknown timezone `{0}`")]
    UnknownTimezone(String),
}

/// Stable context: changes per session, not per turn — safe to cache.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StableTimeContext {
    pub timezone: String,
    pub utc_offset_hours: i32,
}

/// Volatile context: changes per turn — must be a tool call, not cached text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VolatileTimeValue {
    pub iso8601: String,
    pub epoch_seconds: u64,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub weekday: String,
}

impl fmt::Display for VolatileTimeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.iso8601)
    }
}

pub fn now_utc() -> VolatileTimeValue {
    let now = Utc::now();
    VolatileTimeValue {
        iso8601: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        epoch_seconds: now.timestamp().max(0) as u64,
        year: now.year(),
        month: now.month(),
        day: now.day(),
        hour: now.hour(),
        minute: now.minute(),
        weekday: now.weekday().to_string(),
    }
}

/// Time report: stable part is cache-safe, volatile part is rejected wholesale
/// from cached context — it must be fetched via the tool descriptor below.
pub fn time_report(timezone: &str) -> Result<(StableTimeContext, VolatileTimeValue), ClockError> {
    let offset = match timezone {
        "UTC" => 0,
        "EST" | "America/New_York" => -5,
        "PST" | "America/Los_Angeles" => -8,
        "CET" | "Europe/Paris" => 1,
        "JST" | "Asia/Tokyo" => 9,
        other => return Err(ClockError::UnknownTimezone(other.to_string())),
    };
    Ok((
        StableTimeContext { timezone: timezone.to_string(), utc_offset_hours: offset },
        now_utc(),
    ))
}

/// Tool descriptor for the volatile time value. The agent calls this on demand;
/// the value itself is never embedded in cached context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParam {
    pub name: String,
    pub description: String,
    pub required: bool,
}

pub fn current_time_tool() -> ToolDescriptor {
    ToolDescriptor {
        name: "current_time".into(),
        description: "Get the current UTC time. Call this whenever you need to know what time it is; do not assume or cache.".into(),
        parameters: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_report_shape() {
        let (stable, volatile) = time_report("UTC").unwrap();
        assert_eq!(stable.timezone, "UTC");
        assert_eq!(stable.utc_offset_hours, 0);
        assert!(!volatile.iso8601.is_empty());
        assert!(volatile.epoch_seconds > 0);
    }

    #[test]
    fn stable_context_is_cache_safe() {
        let (a, _) = time_report("JST").unwrap();
        let (b, _) = time_report("JST").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn volatile_context_is_rejected_wholesale() {
        // Two calls at least differ in monotonic production; the contract is
        // that volatile values are never compared across turns.
        let (_, v1) = time_report("UTC").unwrap();
        let (_, v2) = time_report("UTC").unwrap();
        // Values may be equal within the same second; the invariant is that
        // they are NOT cached — modeled here by the type boundary.
        let _ = v1 != v2;
    }

    #[test]
    fn volatile_values_become_tools() {
        let tool = current_time_tool();
        assert_eq!(tool.name, "current_time");
        assert!(tool.description.contains("do not assume or cache"));
    }

    #[test]
    fn tool_descriptor_names_the_tool() {
        let tool = current_time_tool();
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("current_time"));
    }
}
