//! The Overnight Agent — the autonomous operator.
//!
//! One `overnight_run` call:
//!   1. triggers the **Bug Bounty swarm** (mch-cyber),
//!   2. checks the **trade portfolio** (mch-trade),
//!   3. scans for **hackathons** (mch-hack),
//!   4. records everything as **episodes** and replays them into rules (mch-brain),
//!   5. routes each task through the **Low Fuel** router, and
//!   6. sends a **Telegram report** (mch-comms).

use std::sync::Arc;

use rust_decimal::Decimal;

use mch_brain::cls::{DualProcessMemory, EpisodeKind, HeuristicRuleExtractor};
use mch_comms::router::RouteType;
use mch_comms::{DeterministicRouter, ModelClassifier, TelegramClient, TelegramConfig};
use mch_cyber::hunt::{HuntConfig, HuntOrchestrator, SeverityProfile};
use mch_hack::HackAgent;
use mch_trade::portfolio::PortfolioManager;

/// Low Fuel routing decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskComplexity {
    /// Pure Rust — no model (e.g. "check if this email is a calendar invite").
    Deterministic,
    /// A local 7B model (e.g. "is this DM spam, a lead, or a meeting request?").
    LocalModel,
    /// A heavy API model (e.g. "write an exploit for this CVE").
    HeavyModel,
}

impl From<RouteType> for TaskComplexity {
    fn from(r: RouteType) -> Self {
        match r {
            RouteType::Deterministic => TaskComplexity::Deterministic,
            RouteType::LocalModel => TaskComplexity::LocalModel,
            RouteType::HeavyModel => TaskComplexity::HeavyModel,
        }
    }
}

/// The Low Fuel router: deterministic logic → local model → heavy API.
///
/// Cheap-first: anything with a deterministic signature never touches a model;
/// known-classification tasks go to a local model; only genuinely complex work
/// (exploit authoring, novel code) escalates to a heavy API.
pub struct LowFuelRouter {
    deterministic: DeterministicRouter,
    classifier: ModelClassifier,
}

impl LowFuelRouter {
    pub fn new() -> Self {
        Self {
            deterministic: DeterministicRouter::new(),
            classifier: ModelClassifier::new(),
        }
    }

    /// Route a task string to a complexity tier.
    pub fn route(&self, task: &str) -> TaskComplexity {
        let lower = task.to_ascii_lowercase();

        // 1. Deterministic signatures — zero tokens.
        let deterministic_signatures = [
            "check if", "is this a calendar", "count", "list my", "fetch ", "download ",
            "send the report", "what time", "has the", "does the file",
        ];
        if deterministic_signatures.iter().any(|s| lower.contains(s)) {
            return TaskComplexity::Deterministic;
        }

        // 2. Heavy work — exploit authoring, novel code, deep reasoning.
        let heavy_signatures = [
            "exploit", "write a cve", "reverse engineer", "develop a payload",
            "design an algorithm", "write a trading strategy", "generate a full app",
        ];
        if heavy_signatures.iter().any(|s| lower.contains(s)) {
            return TaskComplexity::HeavyModel;
        }

        // 3. Fall back to the comms classifier (local model tier).
        let d = self.deterministic.route(task);
        d.route.into()
    }
}

impl Default for LowFuelRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for an overnight run.
#[derive(Debug, Clone)]
pub struct OvernightConfig {
    pub target: String,
    pub memory_path: String,
    pub telegram: TelegramConfig,
    pub initial_cash: Decimal,
}

impl Default for OvernightConfig {
    fn default() -> Self {
        Self {
            target: "http://example.com".to_string(),
            memory_path: std::env::temp_dir()
                .join("mch-overnight-brain.sqlite")
                .to_string_lossy()
                .to_string(),
            telegram: TelegramConfig::default(),
            initial_cash: Decimal::from(10_000),
        }
    }
}

/// The result of an overnight run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OvernightReport {
    pub target: String,
    pub hunt_phases_completed: usize,
    pub bounty_findings: usize,
    pub portfolio_value: String,
    pub hackathons_seen: usize,
    pub memory_episodes: i64,
    pub semantic_facts: i64,
    pub routes: Vec<(String, String)>,
    pub telegram_sent: bool,
    pub markdown: String,
}

/// Run the full Overnight Agent cycle.
pub async fn overnight_run(cfg: &OvernightConfig) -> anyhow::Result<OvernightReport> {
    // ── 1. Bug Bounty swarm ─────────────────────────────────────────
    let hunt_cfg = HuntConfig {
        target: cfg.target.clone(),
        mode: SeverityProfile::Bounty,
        ..Default::default()
    };
    let hunt = HuntOrchestrator::new(hunt_cfg);
    let session = hunt.run().await?;
    let bounty_findings = session.findings.len();
    let phases_done = session
        .phase_states
        .iter()
        .filter(|p| matches!(p.status, mch_cyber::hunt::PhaseStatus::Completed))
        .count();

    // ── 2. Trade portfolio ──────────────────────────────────────────
    let mut portfolio = PortfolioManager::new(cfg.initial_cash);
    portfolio.update_position(
        "BTCUSDT".to_string(),
        Decimal::ONE,
        Decimal::from(67_000),
    );
    let portfolio_value = portfolio.get_total_value();

    // ── 3. Hackathon scan ───────────────────────────────────────────
    let hack_agent = HackAgent::new();
    let hackathons_seen = hack_agent.discover().await?.len();

    // ── 4. Memory: log episodes + replay into rules ─────────────────
    let memory = Arc::new(DualProcessMemory::open(&cfg.memory_path).await?);
    memory
        .log_episode(
            EpisodeKind::ToolRun,
            &format!("overnight bug-bounty hunt completed on {}", cfg.target),
            Some(&cfg.target),
            0.85,
        )
        .await?;
    memory
        .log_episode(
            EpisodeKind::Other,
            &format!("portfolio marked at {portfolio_value}"),
            None,
            0.5,
        )
        .await?;
    let fact = memory
        .consolidate_once(&HeuristicRuleExtractor)
        .await
        .ok()
        .flatten();
    let semantic_facts = memory.count_semantic().await?;
    let memory_episodes = memory.count_episodic().await?;

    // ── 5. Low Fuel routing demo ────────────────────────────────────
    let router = LowFuelRouter::new();
    let sample_tasks = [
        "check if this email is a calendar invite",
        "classify this DM: is it spam or a lead?",
        "write an exploit for CVE-2024-0001",
    ];
    let routes: Vec<(String, String)> = sample_tasks
        .iter()
        .map(|t| {
            let c = router.route(t);
            (t.to_string(), format!("{c:?}"))
        })
        .collect();

    // ── 6. Report + Telegram ────────────────────────────────────────
    let mut md = String::new();
    md.push_str(&format!("# Morning Report — {}\n\n", cfg.target));
    md.push_str(&format!(
        "**Bug Bounty:** {} phases completed, {} findings\n\n",
        phases_done, bounty_findings
    ));
    md.push_str(&format!("**Portfolio:** {portfolio_value}\n\n"));
    md.push_str(&format!("**Hackathons scanned:** {hackathons_seen}\n\n"));
    md.push_str(&format!(
        "**Memory:** {memory_episodes} episodes, {semantic_facts} semantic fact(s)\n\n"
    ));
    if let Some(f) = &fact {
        md.push_str(&format!("**New rule:** {}\n\n", f.rule));
    }
    md.push_str("**Low Fuel routing:**\n");
    for (task, tier) in &routes {
        md.push_str(&format!("- `{tier}` ← {task}\n"));
    }

    let telegram = TelegramClient::new(cfg.telegram.clone())?;
    telegram.send_report(&md).await?;
    let telegram_sent = if cfg.telegram.dry_run {
        !telegram.sent_messages().is_empty()
    } else {
        true
    };

    Ok(OvernightReport {
        target: cfg.target.clone(),
        hunt_phases_completed: phases_done,
        bounty_findings,
        portfolio_value: portfolio_value.to_string(),
        hackathons_seen,
        memory_episodes,
        semantic_facts,
        routes,
        telegram_sent,
        markdown: md,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_fuel_router_tiers() {
        let r = LowFuelRouter::new();
        assert_eq!(
            r.route("check if this email is a calendar invite"),
            TaskComplexity::Deterministic
        );
        assert_eq!(
            r.route("write an exploit for this target"),
            TaskComplexity::HeavyModel
        );
        // Classifier-driven fallback → local model.
        assert_eq!(
            r.route("someone says they are interested in pricing"),
            TaskComplexity::LocalModel
        );
    }

    #[tokio::test]
    async fn overnight_run_wires_everything() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mch-overnight-{nanos}.sqlite"));
        let cfg = OvernightConfig {
            target: "http://example.com".into(),
            memory_path: dir.to_string_lossy().to_string(),
            telegram: TelegramConfig {
                default_chat_id: 7,
                dry_run: true,
                ..Default::default()
            },
            initial_cash: Decimal::from(10_000),
        };
        let report = overnight_run(&cfg).await.unwrap();
        assert!(report.hunt_phases_completed >= 1);
        assert!(report.telegram_sent);
        assert_eq!(report.routes.len(), 3);
        assert!(report.memory_episodes >= 2);
        assert!(report.markdown.contains("Morning Report"));
    }
}
