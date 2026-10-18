//! mch-compaction — auto-compaction of long session histories.
//!
//! Mined from OpenWorker (github.com/andrewng-code/openworker, MIT, © 2024
//! Andrew Ng), `coworker/compaction.py` (OPE-27 / OPE-189): trigger math,
//! boundary picking, and mechanical working-state extraction. The transcript
//! is never modified — only the outbound view is compacted. Re-implemented in
//! Rust; logic re-derived, not copied.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_THRESHOLD_PCT: f64 = 0.8;
pub const DEFAULT_CAP_TOKENS: u64 = 250_000;
pub const DEFAULT_CONTEXT_WINDOW: u64 = 128_000;
/// Newest slice kept verbatim, as a fraction of the trigger.
pub const KEEP_RECENT_FRACTION: f64 = 0.25;

/// One tool call as it appears in an assistant message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

/// A minimal session message. Tool-role messages carry the result of a call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into(), tool_calls: None, tool_call_id: None }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into(), tool_calls: None, tool_call_id: None }
    }
    pub fn tool_result(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self { role: "tool".into(), content: content.into(), tool_calls: None, tool_call_id: Some(id.into()) }
    }
}

/// chars/4 over serialized messages — the fallback token signal.
pub fn estimate_tokens(messages: &[Message]) -> u64 {
    let total: usize = messages.iter().map(|m| serde_json::to_string(m).map(|s| s.len()).unwrap_or(m.content.len())).sum();
    (total / 4) as u64
}

/// min(threshold_pct × window, cap): the cap exists so 1M-context models
/// compact early — quality degrades well before the nominal limit.
pub fn trigger_tokens(context_window: Option<u64>, threshold_pct: f64, cap_tokens: u64) -> u64 {
    let window = context_window.unwrap_or(DEFAULT_CONTEXT_WINDOW);
    ((threshold_pct * window as f64) as u64).min(cap_tokens)
}

pub fn should_compact(signal: u64, context_window: Option<u64>, threshold_pct: f64, cap_tokens: u64) -> bool {
    signal >= trigger_tokens(context_window, threshold_pct, cap_tokens)
}

/// The canonical index where the verbatim tail begins: the earliest turn start
/// (user message) whose suffix fits the keep budget, falling back to assistant
/// iteration boundaries when a single turn exceeds it (a giant tool loop).
/// None when there is nothing meaningful to summarize.
pub fn pick_boundary(messages: &[Message], keep_tokens: u64) -> Option<usize> {
    let start = usize::from(messages.first().map(|m| m.role == "system").unwrap_or(false));
    let mut users = Vec::new();
    let mut assistants = Vec::new();
    for (i, m) in messages.iter().enumerate().skip(start) {
        match m.role.as_str() {
            "user" => users.push(i),
            "assistant" => assistants.push(i),
            _ => {}
        }
    }

    let fits = |i: usize| estimate_tokens(&messages[i..]) <= keep_tokens;
    let mut boundary = users
        .iter()
        .copied()
        .find(|&i| fits(i))
        // A user boundary with nothing but user messages after it summarizes
        // only the preamble — no work to compact, so keep looking.
        .filter(|&i| messages[i..].iter().any(|m| m.role != "user"));

    if boundary.is_none() && !users.is_empty() {
        // Cut inside the newest turn at an iteration boundary.
        let inside: Vec<usize> = assistants.iter().copied().filter(|&i| i > *users.last().unwrap()).collect();
        boundary = inside.iter().copied().find(|&i| fits(i)).or_else(|| inside.last().copied());
    }
    if boundary.is_none() {
        boundary = assistants.iter().copied().find(|&i| fits(i)).or_else(|| assistants.last().copied());
    }
    boundary.filter(|&b| b > start)
}

/// Mechanical extraction from the span's tool-call records — no LLM, zero
/// hallucination risk: files written, recent commands with exit status,
/// artifacts, tools used.
pub fn extract_working_state(span: &[Message]) -> String {
    const WRITE_HINTS: [&str; 6] = ["write", "edit", "append", "save", "create", "patch"];
    const ARTIFACT_HINTS: [&str; 3] = ["artifact", "publish", "deploy"];

    let results: std::collections::HashMap<&str, &str> = span
        .iter()
        .filter_map(|m| m.tool_call_id.as_deref().map(|id| (id, m.content.as_str())))
        .collect();

    let mut files: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut artifacts: Vec<String> = Vec::new();
    let mut tools: Vec<String> = Vec::new();

    for msg in span {
        for tc in msg.tool_calls.iter().flatten() {
            if !tools.iter().any(|t| t == &tc.name) {
                tools.push(tc.name.clone());
            }
            let lowered = tc.name.to_lowercase();
            let path = tc.arguments.get("path").or_else(|| tc.arguments.get("file_path")).and_then(Value::as_str);
            if let Some(p) = path {
                if WRITE_HINTS.iter().any(|h| lowered.contains(h)) {
                    files.push(p.to_string());
                }
            }
            if lowered == "run_shell" {
                if let Some(cmd) = tc.arguments.get("command").and_then(Value::as_str) {
                    let status = results.get(tc.id.as_str()).map(|r| result_status(r)).unwrap_or_default();
                    let line: String = cmd.split_whitespace().collect::<Vec<_>>().join(" ");
                    let mut line = line.chars().take(160).collect::<String>();
                    if !status.is_empty() {
                        line.push_str(&format!("  [{status}]"));
                    }
                    commands.push(line);
                }
            }
            if ARTIFACT_HINTS.iter().any(|h| lowered.contains(h)) {
                let loc = tc.arguments.get("url").or_else(|| tc.arguments.get("path")).or_else(|| tc.arguments.get("title")).and_then(Value::as_str);
                if let Some(loc) = loc {
                    artifacts.push(loc.to_string());
                }
            }
        }
    }

    let mut lines = vec!["## Working state (extracted mechanically from tool records)".to_string()];
    let written: Vec<String> = dedupe_recent_first(files, 20);
    if !written.is_empty() {
        lines.push("Files written/edited (most recent first):".into());
        lines.extend(written.iter().map(|p| format!("- {p}")));
    }
    let recent: Vec<String> = commands.iter().rev().take(10).rev().cloned().collect();
    if !recent.is_empty() {
        lines.push("Recent shell commands:".into());
        lines.extend(recent.iter().map(|c| format!("- {c}")));
    }
    let made = dedupe_recent_first(artifacts, 10);
    if !made.is_empty() {
        lines.push("Artifacts produced:".into());
        lines.extend(made.iter().map(|a| format!("- {a}")));
    }
    if !tools.is_empty() {
        tools.sort();
        lines.push(format!("Tools used in the summarized span: {}", tools.join(", ")));
    }
    if lines.len() == 1 {
        String::new()
    } else {
        lines.join("\n")
    }
}

fn result_status(result: &str) -> String {
    let Ok(parsed) = serde_json::from_str::<Value>(result) else { return String::new() };
    let Some(obj) = parsed.as_object() else { return String::new() };
    if obj.get("error").map(|e| !e.is_null()).unwrap_or(false) {
        return "error".into();
    }
    match obj.get("exit_code") {
        Some(Value::Number(n)) if n.as_i64() == Some(0) => "ok".into(),
        Some(Value::Number(n)) => format!("exit {n}"),
        Some(Value::String(s)) if s == "0" => "ok".into(),
        Some(Value::String(s)) => format!("exit {s}"),
        _ => String::new(),
    }
}

fn dedupe_recent_first(items: Vec<String>, limit: usize) -> Vec<String> {
    let mut seen = Vec::new();
    for item in items.into_iter().rev() {
        if !seen.contains(&item) {
            seen.push(item);
        }
        if seen.len() >= limit {
            break;
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn shell_call(id: &str, cmd: &str) -> Message {
        Message {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: Some(vec![ToolCall { id: id.into(), name: "run_shell".into(), arguments: json!({"command": cmd}) }]),
            tool_call_id: None,
        }
    }

    #[test]
    fn trigger_math() {
        assert_eq!(trigger_tokens(Some(128_000), 0.8, 250_000), 102_400);
        assert_eq!(trigger_tokens(Some(1_000_000), 0.8, 250_000), 250_000);
        assert_eq!(trigger_tokens(None, 0.8, 250_000), 102_400);
    }

    #[test]
    fn estimate_is_chars_over_four() {
        let msgs = vec![Message::user("12345678")]; // 8 chars + role/serde overhead
        assert!(estimate_tokens(&msgs) > 0);
    }

    #[test]
    fn boundary_prefers_earliest_fitting_turn() {
        let msgs = vec![
            Message::assistant("sys"),
            Message::user("turn one mandate"),
            Message::assistant("work"),
            Message::user("turn two"),
            Message::assistant("more"),
        ];
        // Keeping everything fits → earliest user boundary (index 1).
        assert_eq!(pick_boundary(&msgs, 1_000), Some(1));
    }

    #[test]
    fn boundary_falls_back_inside_giant_turn() {
        let big = "x".repeat(10_000);
        let msgs = vec![
            Message::assistant("sys"),
            Message::user(big),
            Message::assistant("step a"),
            Message::assistant("step b"),
        ];
        // A 10k-char user turn alone exceeds a tiny budget → cut inside it,
        // keeping the last assistant step.
        assert_eq!(pick_boundary(&msgs, 10), Some(3));
    }

    #[test]
    fn boundary_none_when_nothing_to_summarize() {
        let msgs = vec![Message::assistant("sys"), Message::user("only turn")];
        assert_eq!(pick_boundary(&msgs, 1_000), None);
    }

    #[test]
    fn working_state_extracts_files_commands_tools() {
        let span = vec![
            shell_call("1", "nmap -sV 10.0.0.1"),
            Message::tool_result("1", r#"{"exit_code":0}"#),
            Message {
                role: "assistant".into(),
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "2".into(),
                    name: "write_file".into(),
                    arguments: json!({"path": "/tmp/findings.json"}),
                }]),
                tool_call_id: None,
            },
        ];
        let state = extract_working_state(&span);
        assert!(state.contains("run_shell"));
        assert!(state.contains("nmap -sV 10.0.0.1  [ok]"));
        assert!(state.contains("/tmp/findings.json"));
    }

    #[test]
    fn working_state_empty_when_no_tool_calls() {
        assert_eq!(extract_working_state(&[Message::user("hi")]), "");
    }
}
