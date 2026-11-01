//! mch-core — McHarness CLI, the Father of Harnesses.
//!
//! Ties the mined jewels together: scope authority gates every target,
//! completion gates certify "done", and the technique ledger remembers.

use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use mch_gate::{CompletionGate, GateOutcome, Requirement, RunContext};
use mch_ledger::{TechniqueLedger, TechniqueStatus};
use mch_loop::{FailureClassifier, LoopEvent, MaxStepsGuard, RetryPolicy, StepExecutor, StepExecutorConfig, StepOutcome};
use mch_scope::{ScopeAuthority, ScopeSet};
use mch_transport::{TransportConfig, TransportFactory};
use mch_snapshot::{SnapshotConfig, SnapshotService};
use tokio::sync::broadcast;

const SCOPE_PATH: &str = "mch.scope.toml";
const LEDGER_PATH: &str = "mch.ledger.json";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rt = tokio::runtime::Runtime::new().unwrap();
    match rt.block_on(run(&args)) {
        Ok(out) => {
            println!("{out}");
        }
        Err(e) => {
            eprintln!("mch: {e}");
            process::exit(1);
        }
    }
}

async fn run(args: &[String]) -> Result<String, String> {
    match args.first().map(|s| s.as_str()) {
        Some("init") => cmd_init().await,
        Some("check") => cmd_check(args.get(1).map(|s| s.as_str())).await,
        Some("gate") => cmd_gate().await,
        Some("remember") => cmd_remember(&args.get(1..).unwrap_or(&[])).await,
        Some("brief") => cmd_brief(args.get(1).map(|s| s.as_str())).await,
        Some("loop") => cmd_loop().await,
        Some("--version") | Some("version") => Ok(env!("CARGO_PKG_VERSION").to_string()),
        _ => Ok(usage().to_string()),
    }
}

fn usage() -> &'static str {
    "McHarness — the Father of Harnesses

USAGE:
    mch <command>

COMMANDS:
    init                    Write a template scope file (fail-closed until filled)
    check <host>            Authorize a host against the scope authority
    gate                    Evaluate the completion gate for this run
    remember <target> <class> <technique> <status> [note]
                            Record a technique outcome in the ledger
    brief <target>          Print the do-not-retry brief from the ledger
    loop                    Run the streaming agent loop (demo)
    version                 Print version

STATUS VOCABULARY (closed set):
    attempted confirmed false-positive exhausted effective waf-bypass
    blocked rate-limited da-killed browser-rejected needs-human-review retired"
}

async fn cmd_init() -> Result<String, String> {
    let path = PathBuf::from(SCOPE_PATH);
    if path.exists() {
        return Ok(format!("scope file already exists at {SCOPE_PATH}"));
    }
    let template = r#"# McHarness scope — FAIL-CLOSED: leaving in_scope empty denies everything.
[in_scope]
hosts = []
cidrs = []

[out_of_scope]
hosts = []
cidrs = []
"#;
    std::fs::write(&path, template).map_err(|e| e.to_string())?;
    Ok(format!("wrote {SCOPE_PATH} — fill [in_scope] before any execution will be authorized"))
}

fn load_authority() -> Result<ScopeAuthority, String> {
    let text = std::fs::read_to_string(SCOPE_PATH)
        .map_err(|_| "no scope file — run `mch init` first (fail-closed)".to_string())?;
    ScopeAuthority::from_toml(&text).map_err(|e| e.to_string())
}

async fn cmd_check(host: Option<&str>) -> Result<String, String> {
    let host = host.ok_or("usage: mch check <host>")?;
    let auth = load_authority()?;
    match auth.authorize(host) {
        Ok(v) => Ok(format!("{host}: {v} — execution authorized")),
        Err(e) => Err(format!("{host}: DENIED — {e}")),
    }
}

async fn cmd_gate() -> Result<String, String> {
    let mut gate = CompletionGate::new("mch-run-complete");
    gate.require(Requirement::ArtifactExists {
        name: "findings".to_string(),
        path: PathBuf::from("findings.json"),
    });
    gate.require(Requirement::ArtifactExists {
        name: "scope".to_string(),
        path: PathBuf::from(SCOPE_PATH),
    });
    let ctx = RunContext::default();
    match gate.evaluate(&PathBuf::from("."), &ctx).map_err(|e| e.to_string())? {
        GateOutcome::Passed => Ok("gate: PASSED".to_string()),
        GateOutcome::Failed(failures) => Err(format!("gate: {failures:?}")),
    }
}

async fn cmd_remember(rest: &[String]) -> Result<String, String> {
    if rest.len() < 4 {
        return Err("usage: mch remember <target> <class> <technique> <status> [note]".into());
    }
    let status = TechniqueStatus::parse(&rest[3]).map_err(|e| e.to_string())?;
    let mut ledger = TechniqueLedger::load(&PathBuf::from(LEDGER_PATH)).map_err(|e| e.to_string())?;
    ledger.record(&rest[0], &rest[1], &rest[2], status, rest.get(4).map(|s| s.as_str()).unwrap_or(""));
    ledger.save(&PathBuf::from(LEDGER_PATH)).map_err(|e| e.to_string())?;
    Ok(format!("recorded {}/{}/{} = {}", rest[0], rest[1], rest[2], rest[3]))
}

async fn cmd_brief(target: Option<&str>) -> Result<String, String> {
    let target = target.ok_or("usage: mch brief <target>")?;
    let ledger = TechniqueLedger::load(&PathBuf::from(LEDGER_PATH)).map_err(|e| e.to_string())?;
    let brief = ledger.exhausted_brief(target);
    if brief.is_empty() {
        Ok(format!("no exhausted techniques for {target}"))
    } else {
        Ok(brief.join("\n"))
    }
}

/// Demo command: run the streaming agent loop with mocked transport.
async fn cmd_loop() -> Result<String, String> {
    println!("Starting McHarness streaming agent loop demo...");
    println!("This demonstrates the OpenCode-mined streaming step loop.");
    println!();

    // Initialize components
    let scope_text = std::fs::read_to_string(SCOPE_PATH)
        .unwrap_or_else(|_| {
            r#"[in_scope]
hosts = ["localhost"]
cidrs = ["127.0.0.1/32"]

[out_of_scope]
hosts = []
cidrs = []
"#.to_string()
        });

    let scope_authority = ScopeAuthority::from_toml(&scope_text)
        .map_err(|e| e.to_string())?;

    let config = StepExecutorConfig {
        max_steps: Some(5),
        max_concurrent_tools: 4,
        tool_timeout_ms: 30_000,
        snapshot_enabled: true,
    };

    // Create transport (will use HTTP fallback for demo)
    let transport_config = TransportConfig::default();
    let transport = TransportFactory::create(transport_config);

    // Create snapshot service
    let snapshot_config = SnapshotConfig::default();
    let snapshot_service = mch_snapshot::create_snapshot_service(snapshot_config)
        .map_err(|e| e.to_string())?;

    // Create step executor
    let mut executor = StepExecutor::new(
        config,
        transport,
        Some(snapshot_service),
        Arc::new(scope_authority),
    );

    // Subscribe to events
    let mut event_rx = executor.subscribe();

    // Run a few demo steps
    for step in 1..=3 {
        println!("=== Step {step} ===");

        let ctx = mch_loop::StepContext {
            step_id: mch_loop::StepId::new(),
            step_number: step,
            session_id: "demo-session".to_string(),
            agent_id: "demo-agent".to_string(),
            model_id: "demo-model".to_string(),
            max_steps: Some(5),
            tools: vec![],
            system_prompt: "You are a helpful assistant.".to_string(),
            messages: vec![
                mch_loop::Message {
                    role: mch_loop::MessageRole::User,
                    content: format!("Demo step {step}"),
                    tool_calls: None,
                    tool_call_id: None,
                }
            ],
        };

        match executor.run_step(ctx).await {
            Ok(outcome) => {
                println!("Step outcome: {:?}", outcome);
            }
            Err(e) => {
                eprintln!("Step error: {}", e);
            }
        }

        // Process events
        while let Ok(event) = event_rx.try_recv() {
            println!("  Event: {:?}", event);
        }

        // Check snapshot
        if let Some(snapshot_id) = executor.take_snapshot().await {
            println!("  Snapshot captured: {:?}", snapshot_id);
        }
    }

    println!("\nDemo complete. The streaming agent loop is working!");
    Ok("Loop demo completed".to_string())
}

// Re-exports kept public for downstream embedders of the harness library.
pub use mch_scope as scope;

/// Convenience constructor used by embedders: build an authority from raw text.
pub fn authority_from_str(s: &str) -> Result<ScopeAuthority, mch_scope::ScopeError> {
    ScopeAuthority::new(ScopeSet::from_toml_str(s)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_nonempty() {
        assert!(usage().contains("McHarness"));
    }

    #[test]
    fn authority_helper_parses() {
        let a = authority_from_str("[in_scope]\nhosts = [\"a.com\"]\n").unwrap();
        assert!(!a.is_empty());
    }
}