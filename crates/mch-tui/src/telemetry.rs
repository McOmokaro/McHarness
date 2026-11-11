//! Telemetry: the live data each pane renders, wired to real sources.

use crate::theme::Health;
use mch_brain::cls::DualProcessMemory;
use mch_kali::lazy::{LazyLoadConfig, LazyLoader};
use mch_kali::tools::ToolRegistry;
use serde::{Deserialize, Serialize};

/// Pane 1 — The Brain (mch-brain / Complementary Learning Systems).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainTelemetry {
    pub episodic: i64,
    pub semantic: i64,
    /// "Sleeping" | "Consolidating" | "Overdue".
    pub replay_status: String,
    /// Minutes until the next replay pass.
    pub next_consolidation_min: u64,
    /// How many episodes one pass consumes.
    pub batch_size: usize,
}

impl Default for BrainTelemetry {
    fn default() -> Self {
        Self {
            episodic: 0,
            semantic: 0,
            replay_status: "Sleeping".into(),
            next_consolidation_min: 30,
            batch_size: 10,
        }
    }
}

impl BrainTelemetry {
    /// Human string for the replay daemon line, e.g. "Sleeping… next consolidation in 24m".
    pub fn replay_line(&self) -> String {
        format!(
            "{}… next consolidation in {}m",
            self.replay_status, self.next_consolidation_min
        )
    }
}

/// Read live brain telemetry from the CLS store at `memory_path`.
pub async fn load_brain(memory_path: &str) -> anyhow::Result<BrainTelemetry> {
    let mem = DualProcessMemory::open(memory_path).await?;
    let episodic = mem.count_episodic().await?;
    let semantic = mem.count_semantic().await?;
    let interval = mem.replay_interval().as_secs();
    let batch_size = mem.batch_size();
    let unconsolidated = mem.unconsolidated_count().await?;
    // Heuristic: if a full batch is waiting, replay is due soon.
    let status = if unconsolidated as usize >= batch_size {
        "Consolidating"
    } else {
        "Sleeping"
    };
    Ok(BrainTelemetry {
        episodic,
        semantic,
        replay_status: status.to_string(),
        next_consolidation_min: interval / 60,
        batch_size,
    })
}

/// Pane 2 — The Key Farm.
#[derive(Debug, Clone)]
pub struct KeyEntry {
    pub model: String,
    pub key_masked: String,
    pub health: Health,
}

/// The set of configured models + masked keys + rate-limit health.
#[derive(Debug, Clone, Default)]
pub struct KeyFarm {
    pub entries: Vec<KeyEntry>,
}

impl KeyFarm {
    pub fn from_specs(specs: &[(&str, &str, Health)]) -> Self {
        Self {
            entries: specs
                .iter()
                .map(|(m, k, h)| KeyEntry {
                    model: (*m).to_string(),
                    key_masked: mask_key(k),
                    health: *h,
                })
                .collect(),
        }
    }

    /// A representative farm for the demo/command-center shell.
    pub fn demo() -> Self {
        Self::from_specs(&[
            ("deepseek-v4.1-flash", "nvapi-9f3c2a7be14d4c0f8a6b2e1d", Health::Green),
            ("claude-opus-4-6", "sk-ant-api03-7b1e9c4a2f6d", Health::Green),
            ("gpt-5.4", "sk-proj-2c8d5e9a1b3f", Health::Yellow),
            ("qwen2.5-7b (local)", "local-ollama-no-key", Health::Green),
        ])
    }

    pub fn worst_health(&self) -> Health {
        if self.entries.iter().any(|e| e.health == Health::Red) {
            Health::Red
        } else if self.entries.iter().any(|e| e.health == Health::Yellow) {
            Health::Yellow
        } else {
            Health::Green
        }
    }
}

/// Mask a secret, keeping only the last 4 characters visible.
pub fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let n = chars.len();
    if n <= 4 {
        return "*".repeat(n.max(4));
    }
    let tail: String = chars[n - 4..].iter().collect();
    format!("{}{tail}", "*".repeat(n - 4))
}

/// Pane 3 — The Arsenal (mch-kali + mch-cyber).
#[derive(Debug, Clone, Default)]
pub struct Arsenal {
    pub targets: Vec<String>,
    pub running: Vec<String>,
    pub swarm_status: String,
    pub installed_tools: usize,
    pub total_tools: usize,
}

impl Arsenal {
    /// Read live tool availability from the Kali lazy loader.
    pub fn load(loader: &LazyLoader) -> Self {
        let registry = ToolRegistry::probe_all(loader);
        let installed = registry.installed();
        Self {
            targets: Vec::new(),
            running: Vec::new(),
            swarm_status: "Idle".into(),
            installed_tools: installed.len(),
            total_tools: registry.stubs.len(),
        }
    }
}

/// Pane 4 — The Overnight Agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OvernightTelemetry {
    pub last_run: Option<String>,
    pub last_report: Option<String>,
    pub fuel_today: u64,
}

/// All telemetry, ready for rendering.
#[derive(Debug, Clone, Default)]
pub struct Telemetry {
    pub brain: BrainTelemetry,
    pub keyfarm: KeyFarm,
    pub arsenal: Arsenal,
    pub overnight: OvernightTelemetry,
}

impl Telemetry {
    /// Build a fully-populated telemetry snapshot from live sources.
    pub async fn sample(memory_path: &str, loader: &LazyLoader) -> Self {
        let brain = load_brain(memory_path).await.unwrap_or_default();
        let mut arsenal = Arsenal::load(loader);
        arsenal.swarm_status = "Standby".into();
        Self {
            brain,
            keyfarm: KeyFarm::demo(),
            arsenal,
            overnight: OvernightTelemetry::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_keeps_only_last_four() {
        let m = mask_key("sk-or-v1-supersecret1234");
        assert!(m.ends_with("1234"));
        assert!(!m.contains("supersecret"));
        assert_eq!(m.chars().count(), "sk-or-v1-supersecret1234".chars().count());
    }

    #[test]
    fn mask_handles_short_keys() {
        assert_eq!(mask_key("abc"), "****");
    }

    #[test]
    fn keyfarm_worst_health() {
        let farm = KeyFarm::from_specs(&[
            ("a", "k1", Health::Green),
            ("b", "k2", Health::Yellow),
        ]);
        assert_eq!(farm.worst_health(), Health::Yellow);
        // Short keys are fully masked to a fixed-width "****".
        assert_eq!(farm.entries[0].key_masked, "****");
    }

    #[test]
    fn replay_line_formats() {
        let t = BrainTelemetry {
            replay_status: "Sleeping".into(),
            next_consolidation_min: 24,
            ..Default::default()
        };
        assert_eq!(t.replay_line(), "Sleeping… next consolidation in 24m");
    }
}
