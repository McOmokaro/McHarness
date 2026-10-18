//! mch-audit — durable audit events with mandatory redaction.
//!
//! Mined from OpenWorker (github.com/andrewng-code/openworker, MIT, © 2024
//! Andrew Ng), `coworker/audit.py` (issues #397, #525): a tool RESULT carries
//! the same content the argument policy redacts under other names, so both
//! sides are covered, and redaction runs at EVERY level of a nested structure
//! so an HTTP-shaped tool's bearer token in `headers` cannot slip through
//! whole. Re-implemented in Rust; logic re-derived, not copied.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Matched as substrings of a lowercased key name, at every depth.
pub const SECRET_KEYS: [&str; 12] = [
    "token",
    "secret",
    "password",
    "api_key",
    "access_token",
    "bot_token",
    "app_token",
    "authorization",
    "cookie",
    "credential",
    "private_key",
    "raw",
];

/// Content-bearing argument keys.
pub const BODY_KEYS: [&str; 3] = ["body", "content", "html"];
/// A tool result's triage-side content keys (stdout etc. are for replay, not triage).
pub const RESULT_BODY_KEYS: [&str; 8] = ["body", "content", "html", "output", "stdout", "stderr", "text", "snippet"];

/// How deep the walk goes before it stops describing a structure: past this,
/// the keys are no longer being checked, so the value is dropped rather than
/// copied through.
pub const MAX_DEPTH: usize = 6;

pub const PREVIEW_LIMIT: usize = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub resource: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub result: Value,
    #[serde(default)]
    pub tokens_in: u64,
    #[serde(default)]
    pub tokens_out: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizedEvent {
    pub tool: String,
    pub status: String,
    pub reason: String,
    pub resource: String,
    pub arguments: Value,
    pub result_preview: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

/// Redact one event's arguments and result under the argument-side and
/// result-side key policies respectively.
pub fn sanitize_event(ev: &AuditEvent) -> SanitizedEvent {
    SanitizedEvent {
        tool: ev.tool.clone(),
        status: ev.status.clone(),
        reason: truncate(&ev.reason),
        resource: truncate(&ev.resource),
        arguments: sanitize_value(&ev.arguments, &BODY_KEYS, 0),
        result_preview: result_preview(&ev.result),
        tokens_in: ev.tokens_in,
        tokens_out: ev.tokens_out,
    }
}

fn truncate(s: &str) -> String {
    s.chars().take(PREVIEW_LIMIT).collect()
}

/// The marker this key's value must be replaced by, or None to keep it.
fn redaction_marker(lower_key: &str, body_keys: &[&str]) -> Option<&'static str> {
    if SECRET_KEYS.iter().any(|s| lower_key.contains(s)) {
        return Some("[redacted]");
    }
    if body_keys.iter().any(|b| lower_key == *b || lower_key.ends_with(&format!("_{b}"))) {
        return Some("[redacted body]");
    }
    None
}

/// Recursion with a hard depth cap — past MAX_DEPTH the value is dropped
/// rather than copied through unchecked.
pub fn sanitize_value(value: &Value, body_keys: &[&str], depth: usize) -> Value {
    if depth >= MAX_DEPTH {
        return Value::String("[nested]".into());
    }
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, val) in map.iter().take(20) {
                let lower = key.to_lowercase();
                if let Some(marker) = redaction_marker(&lower, body_keys) {
                    out.insert(key.clone(), Value::String(marker.into()));
                } else {
                    out.insert(key.clone(), sanitize_value(val, body_keys, depth + 1));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => {
            Value::Array(items.iter().take(10).map(|v| sanitize_value(v, body_keys, depth + 1)).collect())
        }
        Value::String(s) => Value::String(truncate(s)),
        other => other.clone(),
    }
}

/// The result preview for triage: redacted under the result-side keys, then
/// clipped to the preview limit.
pub fn result_preview(result: &Value) -> String {
    let sanitized = sanitize_value(result, &RESULT_BODY_KEYS, 0);
    let text = match &sanitized {
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    };
    text.chars().take(PREVIEW_LIMIT).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(tool: &str, arguments: Value, result: Value) -> AuditEvent {
        AuditEvent {
            tool: tool.into(),
            status: "ok".into(),
            reason: String::new(),
            resource: String::new(),
            arguments,
            result,
            tokens_in: 0,
            tokens_out: 0,
        }
    }

    #[test]
    fn bearer_token_in_nested_headers_is_redacted() {
        // The #397 class: an HTTP-shaped tool takes its credential in a nested
        // `headers` object; an unredacted log lands the token whole.
        let e = ev(
            "http_request",
            json!({"url": "https://example.com", "headers": {"Authorization": "Bearer sk-live-123", "X-Other": "fine"}}),
            json!({}),
        );
        let s = sanitize_event(&e);
        let headers = s.arguments.get("headers").unwrap();
        assert_eq!(headers.get("Authorization").unwrap(), "[redacted]");
        assert_eq!(headers.get("X-Other").unwrap(), "fine");
    }

    #[test]
    fn shell_stdout_in_result_is_redacted() {
        // The #525 class: a tool RESULT carries the same content under other
        // names — stdout is for replay, not triage.
        let e = ev("run_shell", json!({"command": "cat /etc/passwd"}), json!({"exit_code": 0, "stdout": "root:..."}));
        let preview = result_preview(&e.result);
        assert!(!preview.contains("root:"));
        assert!(preview.contains("[redacted body]"));
    }

    #[test]
    fn long_strings_are_clipped() {
        let e = ev("t", json!({"data": "a".repeat(1000)}), json!({}));
        let s = sanitize_event(&e);
        let data = s.arguments.get("data").unwrap().as_str().unwrap();
        assert_eq!(data.len(), PREVIEW_LIMIT);
    }

    #[test]
    fn deep_nesting_is_dropped_not_leaked() {
        // Seven levels deep: past MAX_DEPTH the value is dropped rather than
        // copied through unchecked.
        let mut v = json!("deep-secret-value");
        for _ in 0..8 {
            v = json!({"level": v});
        }
        let s = sanitize_value(&v, &BODY_KEYS, 0);
        assert!(!serde_json::to_string(&s).unwrap().contains("deep-secret-value"));
        assert!(serde_json::to_string(&s).unwrap().contains("[nested]"));
    }

    #[test]
    fn marker_under_underscore_suffix_key() {
        // `access_token` style keys that are not exact matches.
        let s = sanitize_value(&json!({"bot_token": "x", "user_content": "y"}), &BODY_KEYS, 0);
        assert_eq!(s.get("bot_token").unwrap(), "[redacted]");
        assert_eq!(s.get("user_content").unwrap(), "[redacted body]");
    }
}
