//! mch-core — McHarness CLI, the Father of Harnesses.
//!
//! Ties the mined jewels together: scope authority gates every target,
//! completion gates certify "done", and the technique ledger remembers.

use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use mch_gate::{CompletionGate, GateOutcome, Requirement, RunContext};
use mch_ledger::{TechniqueLedger, TechniqueStatus};
use mch_loop::{StepExecutor, StepExecutorConfig};
use mch_mcp::{McpClient, ToolCatalog, DEFAULT_TIMEOUT_SECS, duplex_pair};
use mch_scope::{ScopeAuthority, ScopeSet};
use mch_transport::{TransportConfig, TransportFactory};
use mch_snapshot::SnapshotConfig;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

mod overnight;

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
        Some("memory") => cmd_memory().await,
        Some("overnight") => cmd_overnight(args.get(1).map(|s| s.as_str())).await,
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
    memory                  Show the three-strata memory + skill-sealing demo
    overnight [target]      Run the Overnight Agent (bounty+trades+hackathons+report)
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
    println!("This demonstrates the OpenCode-mined streaming step loop with Goose-mined MCP integration.");
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
"#
            .to_string()
        });

    let scope_authority = ScopeAuthority::from_toml(&scope_text)
        .map_err(|e| e.to_string())?;

    let config = StepExecutorConfig {
        max_steps: Some(5),
        max_concurrent_tools: 4,
        tool_timeout_ms: 30_000,
        snapshot_enabled: true,
    };

    // The model transport drives inference in the step loop.
    let transport = TransportFactory::create(TransportConfig::default());

    // The MCP transport (Goose-mined in-process duplex) drives tool execution.
    let (mcp_transport, (_server_read, _server_write)) = duplex_pair();

    // Initialize MCP client and ToolCatalog (Goose-mined integration)
    let mcp_client = McpClient::connect(
        mcp_transport,
        Duration::from_secs(DEFAULT_TIMEOUT_SECS),
    )
    .await
    .map_err(|e| format!("MCP client connect failed: {}", e))?;

    // Build an example ToolCatalog with namespaced tools
    let sources = vec![
        (
            "developer".to_string(),
            vec![mch_mcp::namespace_tool(
                "developer",
                mch_mcp::message::Tool {
                    name: "shell".to_string(),
                    description: Some("Execute shell commands".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "command": { "type": "string" }
                        },
                        "required": ["command"]
                    }),
                    meta: None,
                },
                false,
            )],
        ),
        (
            "filesystem".to_string(),
            vec![mch_mcp::namespace_tool(
                "filesystem",
                mch_mcp::message::Tool {
                    name: "read_file".to_string(),
                    description: Some("Read a file".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" }
                        },
                        "required": ["path"]
                    }),
                    meta: None,
                },
                false,
            )],
        ),
    ];

    let tool_catalog = ToolCatalog::build(sources);
    println!("MCP ToolCatalog built with {} tools:", tool_catalog.entries().len());
    for entry in tool_catalog.entries() {
        println!("  - {} (owner: {})", entry.tool.name, entry.owner);
    }

    // Create snapshot service
    let snapshot_config = SnapshotConfig::default();
    let snapshot_service = mch_snapshot::create_snapshot_service(snapshot_config)
        .map_err(|e| e.to_string())?;

    // Create step executor with MCP integration
    let mut executor = StepExecutor::new(
        config,
        transport,
        Some(snapshot_service),
        Arc::new(scope_authority),
        Some(tool_catalog.clone()),
        Some(Arc::new(mcp_client)),
    );

    // Subscribe to events
    let mut event_rx = executor.subscribe();

    // Run a few demo steps
    for step in 1..=3 {
        println!("\n=== Step {} ===", step);

        let ctx = mch_loop::StepContext {
            step_id: mch_loop::StepId::new(),
            step_number: step,
            session_id: "demo-session".to_string(),
            agent_id: "demo-agent".to_string(),
            model_id: "demo-model".to_string(),
            max_steps: Some(5),
            tools: tool_catalog.to_tool_definitions(),
            system_prompt: "You are a helpful assistant with MCP tools.".to_string(),
            messages: vec![
                mch_loop::Message {
                    role: mch_loop::MessageRole::User,
                    content: format!("Demo step {} - use a tool if needed", step),
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

    println!("\nDemo complete. The streaming agent loop with MCP integration is working!");
    Ok("Loop demo completed".to_string())
}

/// Demo command: exercise the Hermes-mined memory, skill-sealing and curator paths.
async fn cmd_memory() -> Result<String, String> {
    use mch_memory::{
        Curator, LearningGraph, MemoryCard, MemoryOp, MemoryStore, MemoryTarget, SkillNode,
        SkillState, SkillStore,
    };

    let dir = std::env::temp_dir().join("mch-memory-demo");
    let _ = std::fs::remove_dir_all(&dir);

    println!("=== McHarness three-strata memory demo ===");
    println!("(mined from Hermes Agent — no network, no model)\n");

    // Stratum A — persistent, declarative memory (frozen snapshot).
    let mut store = MemoryStore::new(dir.join("memories"));
    store.load_from_disk().map_err(|e| e.to_string())?;
    store
        .add(MemoryTarget::Memory, "user runs McHarness on a rack, prefers terseness")
        .map_err(|e| e.to_string())?;
    println!(
        "after add, LIVE entries = {:?}",
        store.entries(MemoryTarget::Memory)
    );
    println!(
        "frozen prompt snapshot (captured at load) = {:?}  <- unchanged by the write\n",
        store.frozen_snapshot().get(MemoryTarget::Memory)
    );

    // Atomic batch: prune to make room, add the new fact in one call.
    store
        .apply_batch(
            MemoryTarget::Memory,
            &[
                MemoryOp::Remove {
                    old_text: "prefers terseness".into(),
                },
                MemoryOp::Add {
                    content: "user prefers terse replies with exact commands".into(),
                },
            ],
        )
        .map_err(|e| e.to_string())?;
    println!(
        "after batch, LIVE entries = {:?}\n",
        store.entries(MemoryTarget::Memory)
    );

    // Stratum B — skill-sealing.
    let skills = SkillStore::new(dir.join("skills"));
    let body = "---\nname: arxiv-search\ndescription: Search arXiv papers by keyword, author, or ID.\n---\n\n# arXiv search\n\nQuery the arXiv API through the terminal tool.\n";
    skills
        .create("arxiv-search", body, Some("research"))
        .map_err(|e| e.to_string())?;
    println!("sealed skill 'arxiv-search' at {:?}", skills.find("arxiv-search"));

    // Stratum C — the learning loop (curator) + the visible learning graph.
    let now = 1_000_000_000i64;
    let mut records = vec![
        mch_memory::SkillRecord {
            created_by: Some("agent".into()),
            last_activity_at: now - 2 * 86_400,
            use_count: 3,
            ..mch_memory::SkillRecord::new("arxiv-search")
        },
        mch_memory::SkillRecord {
            use_count: 4,
            last_activity_at: now - 40 * 86_400,
            created_at: now - 60 * 86_400,
            ..mch_memory::SkillRecord::new("stale-session-artifact")
        },
    ];
    let report = Curator::default().apply_transitions(&mut records, now);
    println!("\ncurator transitions: {}", serde_json::to_string(&report).map_err(|e| e.to_string())?);
    for r in &records {
        println!("  {} -> {:?}", r.name, r.state);
    }

    let graph = LearningGraph::build(
        vec![SkillNode {
            name: "arxiv-search".into(),
            category: "research".into(),
            created_by: Some("agent".into()),
            use_count: 3,
            related: vec![],
        }],
        vec![MemoryCard::new("memory", "arxiv search finds papers fast")],
    );
    println!(
        "\nlearning graph: {} learned skill(s), {} edge(s)",
        graph.learned_skill_count(),
        graph.edges.len()
    );
    assert_eq!(records[1].state, SkillState::Archived);

    Ok("\nmemory demo complete".to_string())
}

/// Command: run the Overnight Agent cycle (bug bounty → trades → hackathons →
/// memory replay → low-fuel routing → Telegram report).
async fn cmd_overnight(target: Option<&str>) -> Result<String, String> {
    let cfg = overnight::OvernightConfig {
        target: target.unwrap_or("http://example.com").to_string(),
        ..Default::default()
    };
    let report = overnight::overnight_run(&cfg)
        .await
        .map_err(|e| format!("overnight run failed: {e}"))?;
    Ok(report.markdown)
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