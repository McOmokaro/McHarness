//! Complementary Learning Systems (CLS) — a biologically-grounded dual-process brain.
//!
//! The mammalian brain solves the stability/plasticity dilemma with two
//! complementary systems:
//!
//! * **Hippocampus** — fast, sparse, *episodic* memory. Every tool run, target
//!   IP, and chat interaction is written immediately with a salience score.
//! * **Neocortex** — slow, structured, *semantic* memory. General rules
//!   extracted from many episodes, full-text searchable (FTS5), and linked into
//!   an associative graph (`memory_edges`) that strengthens with co-activation
//!   (Hebbian learning).
//!
//! A background **Replay Daemon** (see [`ReplayDaemon`]) periodically pulls the
//! latest un-consolidated episodes, asks an [`RuleExtractor`] (LLM or a
//! deterministic fallback) to distill a single general rule, writes it to the
//! neocortex, and links it back to the episodes that produced it.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// The kind of an episodic memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeKind {
    ToolRun,
    TargetIp,
    Chat,
    Other,
}

impl EpisodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EpisodeKind::ToolRun => "tool_run",
            EpisodeKind::TargetIp => "target_ip",
            EpisodeKind::Chat => "chat",
            EpisodeKind::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> EpisodeKind {
        match s {
            "tool_run" => EpisodeKind::ToolRun,
            "target_ip" => EpisodeKind::TargetIp,
            "chat" => EpisodeKind::Chat,
            _ => EpisodeKind::Other,
        }
    }
}

/// A single episodic memory (hippocampus).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: i64,
    pub kind: EpisodeKind,
    pub content: String,
    pub target: Option<String>,
    pub salience: f64,
    pub consolidated: bool,
    pub created_at: String,
}

/// A distilled semantic rule (neocortex).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFact {
    pub id: i64,
    pub rule: String,
    pub salience: f64,
    /// Episodes this rule was consolidated from.
    pub source_episode_ids: Vec<i64>,
    pub created_at: String,
}

/// A Hebbian edge between two memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEdge {
    pub from_kind: String,
    pub from_id: i64,
    pub to_kind: String,
    pub to_id: i64,
    pub weight: f64,
    pub edge_type: String,
}

/// Configuration for the dual-process memory.
#[derive(Debug, Clone)]
pub struct ClsConfig {
    /// How many episodes a single consolidation pass consumes.
    pub batch_size: usize,
    /// Replay cadence.
    pub replay_interval: Duration,
}

impl Default for ClsConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            replay_interval: Duration::from_secs(30 * 60), // every 30 minutes
        }
    }
}

/// Hook the Replay Daemon calls to turn episodes into one general rule.
///
/// The production implementation wraps an LLM with the prompt
/// *"Extract a single, general semantic rule from these events."* The default
/// [`HeuristicRuleExtractor`] is deterministic so the daemon works offline and
/// is testable without a model.
#[async_trait]
pub trait RuleExtractor: Send + Sync {
    async fn extract_rule(&self, episodes: &[Episode]) -> Option<String>;
}

/// Deterministic, offline rule extractor.
///
/// Distills the most frequent target and the dominant episode kind into a
/// single generalizable statement — a stand-in that lets the daemon (and its
/// tests) run with no model dependency.
pub struct HeuristicRuleExtractor;

#[async_trait]
impl RuleExtractor for HeuristicRuleExtractor {
    async fn extract_rule(&self, episodes: &[Episode]) -> Option<String> {
        if episodes.is_empty() {
            return None;
        }

        // Most frequent target.
        let mut targets: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for e in episodes {
            if let Some(t) = &e.target {
                *targets.entry(t.as_str()).or_insert(0) += 1;
            }
        }
        let top_target = targets
            .iter()
            .max_by_key(|(_, n)| **n)
            .map(|(t, _)| *t)
            .unwrap_or("unknown");

        // Dominant kind.
        let mut kinds: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for e in episodes {
            *kinds.entry(e.kind.as_str()).or_insert(0) += 1;
        }
        let top_kind = kinds
            .iter()
            .max_by_key(|(_, n)| **n)
            .map(|(k, _)| *k)
            .unwrap_or("event");

        let n = episodes.len();
        Some(format!(
            "Recurring pattern: {n} {top_kind} events on {top_target} share a signal — investigate {top_target} first on sight."
        ))
    }
}

/// The dual-process memory engine: hippocampus + neocortex over one SQLite file.
pub struct DualProcessMemory {
    pool: SqlitePool,
    config: ClsConfig,
}

impl DualProcessMemory {
    /// Open (and initialize) a dual-process memory at `path`.
    pub async fn open(path: &str) -> Result<Self> {
        Self::open_with_config(path, ClsConfig::default()).await
    }

    /// Open with an explicit configuration.
    pub async fn open_with_config(path: &str, config: ClsConfig) -> Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
        }
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite://{path}?mode=rwc"))
            .await?;
        let mem = Self { pool, config };
        mem.init().await?;
        Ok(mem)
    }

    /// Initialize all tables.
    pub async fn init(&self) -> Result<()> {
        // Hippocampus.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS episodic_memory (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                content TEXT NOT NULL,
                target TEXT,
                salience REAL NOT NULL DEFAULT 0.5,
                consolidated INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Neocortex.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS semantic_memory (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rule TEXT NOT NULL,
                salience REAL NOT NULL DEFAULT 0.5,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Full-text search over semantic rules.
        sqlx::query(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS semantic_memory_fts USING fts5(
                rule,
                content='semantic_memory',
                content_rowid='id'
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Hebbian associative graph.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS memory_edges (
                from_kind TEXT NOT NULL,
                from_id INTEGER NOT NULL,
                to_kind TEXT NOT NULL,
                to_id INTEGER NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                edge_type TEXT NOT NULL DEFAULT 'hebbian',
                PRIMARY KEY (from_kind, from_id, to_kind, to_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_episodic_consolidated ON episodic_memory(consolidated)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ── Hippocampus ─────────────────────────────────────────────────

    /// Log an episodic memory; returns its id.
    pub async fn log_episode(
        &self,
        kind: EpisodeKind,
        content: &str,
        target: Option<&str>,
        salience: f64,
    ) -> Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        let res = sqlx::query(
            "INSERT INTO episodic_memory (kind, content, target, salience, consolidated, created_at) \
             VALUES (?, ?, ?, ?, 0, ?)",
        )
        .bind(kind.as_str())
        .bind(content)
        .bind(target)
        .bind(salience)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(res.last_insert_rowid())
    }

    /// Convenience: log a tool run.
    pub async fn log_tool_run(&self, tool: &str, target: &str, salience: f64) -> Result<i64> {
        let content = format!("ran tool `{tool}` against {target}");
        self.log_episode(EpisodeKind::ToolRun, &content, Some(target), salience)
            .await
    }

    /// The latest un-consolidated episodes, newest first.
    pub async fn recent_unconsolidated(&self, limit: usize) -> Result<Vec<Episode>> {
        let rows = sqlx::query(
            "SELECT id, kind, content, target, salience, consolidated, created_at \
             FROM episodic_memory WHERE consolidated = 0 ORDER BY id DESC LIMIT ?",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_episode).collect())
    }

    /// Mark episodes as consolidated.
    pub async fn mark_consolidated(&self, ids: &[i64]) -> Result<()> {
        for id in ids {
            sqlx::query("UPDATE episodic_memory SET consolidated = 1 WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn count_episodic(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM episodic_memory")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("c")?)
    }

    pub async fn unconsolidated_count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM episodic_memory WHERE consolidated = 0")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("c")?)
    }

    // ── Neocortex ────────────────────────────────────────────────────

    /// Insert a semantic rule; returns its id.
    pub async fn insert_semantic(&self, rule: &str, salience: f64) -> Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        let res = sqlx::query("INSERT INTO semantic_memory (rule, salience, created_at) VALUES (?, ?, ?)")
            .bind(rule)
            .bind(salience)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        let id = res.last_insert_rowid();
        // Keep FTS in sync (external-content table).
        sqlx::query("INSERT INTO semantic_memory_fts (rowid, rule) VALUES (?, ?)")
            .bind(id)
            .bind(rule)
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn count_semantic(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM semantic_memory")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("c")?)
    }

    /// Full-text search the neocortex.
    pub async fn search_semantic(&self, query: &str, limit: usize) -> Result<Vec<SemanticFact>> {
        let rows = sqlx::query(
            "SELECT s.id, s.rule, s.salience, s.created_at \
             FROM semantic_memory s \
             JOIN semantic_memory_fts f ON s.id = f.rowid \
             WHERE semantic_memory_fts MATCH ? \
             ORDER BY rank LIMIT ?",
        )
        .bind(query)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_fact).collect())
    }

    // ── Hebbian edges ────────────────────────────────────────────────

    /// Create or reinforce an edge (Hebbian: +1 weight on co-activation).
    pub async fn link(&self, from_kind: &str, from_id: i64, to_kind: &str, to_id: i64) -> Result<()> {
        sqlx::query(
            "INSERT INTO memory_edges (from_kind, from_id, to_kind, to_id, weight, edge_type) \
             VALUES (?, ?, ?, ?, 1.0, 'hebbian') \
             ON CONFLICT(from_kind, from_id, to_kind, to_id) \
             DO UPDATE SET weight = weight + 1.0",
        )
        .bind(from_kind)
        .bind(from_id)
        .bind(to_kind)
        .bind(to_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// All edges out of a node.
    pub async fn edges_from(&self, kind: &str, id: i64) -> Result<Vec<MemoryEdge>> {
        let rows = sqlx::query(
            "SELECT from_kind, from_id, to_kind, to_id, weight, edge_type \
             FROM memory_edges WHERE from_kind = ? AND from_id = ?",
        )
        .bind(kind)
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_edge).collect())
    }

    pub async fn count_edges(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM memory_edges")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("c")?)
    }

    // ── Consolidation (Replay) ───────────────────────────────────────

    /// One consolidation pass: pull the latest `batch_size` un-consolidated
    /// episodes, extract a single rule, persist it, and link it back.
    pub async fn consolidate_once(&self, extractor: &dyn RuleExtractor) -> Result<Option<SemanticFact>> {
        let mut episodes = self.recent_unconsolidated(self.config.batch_size).await?;
        if episodes.is_empty() {
            return Ok(None);
        }
        // Oldest-first for a stable narrative.
        episodes.reverse();

        let Some(rule) = extractor.extract_rule(&episodes).await else {
            return Ok(None);
        };

        let avg_salience =
            episodes.iter().map(|e| e.salience).sum::<f64>() / episodes.len() as f64;
        let fact_id = self.insert_semantic(&rule, avg_salience).await?;

        let ids: Vec<i64> = episodes.iter().map(|e| e.id).collect();
        for id in &ids {
            // Hebbian link: semantic fact <-> each source episode.
            self.link("semantic", fact_id, "episodic", *id).await?;
            self.link("episodic", *id, "semantic", fact_id).await?;
        }
        self.mark_consolidated(&ids).await?;

        info!(
            "consolidated {} episodes into semantic fact #{}",
            ids.len(),
            fact_id
        );

        Ok(Some(SemanticFact {
            id: fact_id,
            rule,
            salience: avg_salience,
            source_episode_ids: ids,
            created_at: chrono::Utc::now().to_rfc3339(),
        }))
    }

    /// Access the underlying pool (for advanced callers).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// The configured replay (consolidation) cadence.
    pub fn replay_interval(&self) -> std::time::Duration {
        self.config.replay_interval
    }

    /// The number of episodes consumed per consolidation pass.
    pub fn batch_size(&self) -> usize {
        self.config.batch_size
    }
}

fn row_to_episode(row: &sqlx::sqlite::SqliteRow) -> Episode {
    let kind_s: String = row.try_get("kind").unwrap_or_default();
    Episode {
        id: row.try_get("id").unwrap_or_default(),
        kind: EpisodeKind::from_str(&kind_s),
        content: row.try_get("content").unwrap_or_default(),
        target: row.try_get("target").ok(),
        salience: row.try_get("salience").unwrap_or(0.5),
        consolidated: row.try_get::<i64, _>("consolidated").unwrap_or(0) != 0,
        created_at: row.try_get("created_at").unwrap_or_default(),
    }
}

fn row_to_fact(row: &sqlx::sqlite::SqliteRow) -> SemanticFact {
    SemanticFact {
        id: row.try_get("id").unwrap_or_default(),
        rule: row.try_get("rule").unwrap_or_default(),
        salience: row.try_get("salience").unwrap_or(0.5),
        source_episode_ids: Vec::new(),
        created_at: row.try_get("created_at").unwrap_or_default(),
    }
}

fn row_to_edge(row: &sqlx::sqlite::SqliteRow) -> MemoryEdge {
    MemoryEdge {
        from_kind: row.try_get("from_kind").unwrap_or_default(),
        from_id: row.try_get("from_id").unwrap_or_default(),
        to_kind: row.try_get("to_kind").unwrap_or_default(),
        to_id: row.try_get("to_id").unwrap_or_default(),
        weight: row.try_get("weight").unwrap_or(1.0),
        edge_type: row.try_get("edge_type").unwrap_or_default(),
    }
}

/// The background Replay Daemon.
///
/// Every `replay_interval` it runs a consolidation pass (the biological
/// "hippocampal replay" that writes episodes into cortical rules).
pub struct ReplayDaemon {
    memory: Arc<DualProcessMemory>,
    extractor: Arc<dyn RuleExtractor>,
    interval: Duration,
    batch_size: usize,
}

impl ReplayDaemon {
    pub fn new(memory: Arc<DualProcessMemory>, extractor: Arc<dyn RuleExtractor>) -> Self {
        let interval = memory.config.replay_interval;
        let batch_size = memory.config.batch_size;
        Self {
            memory,
            extractor,
            interval,
            batch_size,
        }
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Run exactly one replay/consolidation pass.
    pub async fn tick(&self) -> Result<Option<SemanticFact>> {
        let _ = self.batch_size; // batch lives on the memory config
        self.memory.consolidate_once(self.extractor.as_ref()).await
    }

    /// Spawn the daemon as a background Tokio task. Runs until the runtime ends.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(self.interval);
            loop {
                ticker.tick().await;
                match self.tick().await {
                    Ok(Some(fact)) => info!("replay consolidated rule #{}: {}", fact.id, fact.rule),
                    Ok(None) => {}
                    Err(e) => warn!("replay pass failed: {e}"),
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh dual-process memory on a unique temp file DB.
    async fn mem() -> DualProcessMemory {
        let path = std::env::temp_dir().join(format!("cls_{}.sqlite", uuid::Uuid::new_v4()));
        DualProcessMemory::open(path.to_str().unwrap()).await.unwrap()
    }

    #[tokio::test]
    async fn replay_consolidates_five_episodes_into_one_fact() {
        let m = Arc::new(mem().await);

        // 5 episodic memories (the hippocampus).
        m.log_tool_run("nmap", "10.0.0.5", 0.6).await.unwrap();
        m.log_tool_run("sqlmap", "10.0.0.5", 0.9).await.unwrap();
        m.log_episode(EpisodeKind::TargetIp, "discovered 10.0.0.5", Some("10.0.0.5"), 0.7)
            .await
            .unwrap();
        m.log_episode(EpisodeKind::Chat, "user asked about 10.0.0.5", Some("10.0.0.5"), 0.4)
            .await
            .unwrap();
        m.log_tool_run("nuclei", "10.0.0.5", 0.8).await.unwrap();

        assert_eq!(m.count_episodic().await.unwrap(), 5);
        assert_eq!(m.count_semantic().await.unwrap(), 0);
        assert_eq!(m.unconsolidated_count().await.unwrap(), 5);

        // One replay pass via the daemon.
        let daemon = ReplayDaemon::new(m.clone(), Arc::new(HeuristicRuleExtractor));
        let fact = daemon.tick().await.unwrap().expect("a rule was distilled");
        assert!(fact.rule.contains("10.0.0.5"));
        assert_eq!(fact.source_episode_ids.len(), 5);

        // 5 episodic → 1 semantic fact; all episodes consolidated.
        assert_eq!(m.count_semantic().await.unwrap(), 1);
        assert_eq!(m.unconsolidated_count().await.unwrap(), 0);
        // Hebbian edges: 5 pairs × 2 directions = 10.
        assert_eq!(m.count_edges().await.unwrap(), 10);
    }

    #[tokio::test]
    async fn fts_search_finds_rule() {
        let m = mem().await;
        m.insert_semantic("sqlmap on 10.0.0.5 reliably yields injectable params", 0.9)
            .await
            .unwrap();
        let hits = m.search_semantic("sqlmap", 5).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].rule.contains("injectable"));
    }

    #[tokio::test]
    async fn consolidation_is_noop_when_empty() {
        let m = mem().await;
        let out = m.consolidate_once(&HeuristicRuleExtractor).await.unwrap();
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn hebbian_edges_reinforce() {
        let m = mem().await;
        m.link("episodic", 1, "semantic", 2).await.unwrap();
        m.link("episodic", 1, "semantic", 2).await.unwrap();
        let edges = m.edges_from("episodic", 1).await.unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].weight, 2.0);
    }
}
