//! The learning loop — background skill maintenance.
//!
//! Mined from Hermes Agent's `agent/curator.py`. It runs on idleness, not a daemon. Two
//! passes: a pure, model-free state transition (active -> stale -> archived), and (optionally)
//! an LLM consolidation fork. The invariants are the interesting part: never delete, only
//! archive; never touch pinned; and `use_count == 0` is absence of evidence, not staleness.

use serde::{Deserialize, Serialize};

/// Lifecycle state of a skill in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillState {
    Active,
    Stale,
    Archived,
}

/// One curator-managed skill's telemetry. `created_by` is `agent` or `learn` when the agent
/// wrote it; telemetry never lives in frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRecord {
    pub name: String,
    pub state: SkillState,
    pub pinned: bool,
    pub use_count: u64,
    pub created_by: Option<String>,
    /// Epoch seconds of the latest real activity; 0 when never used.
    pub last_activity_at: i64,
    /// Epoch seconds of creation; the fallback anchor for a never-active skill.
    pub created_at: i64,
    /// A cron job references this skill, so it is in use by definition.
    pub cron_referenced: bool,
}

impl SkillRecord {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            state: SkillState::Active,
            pinned: false,
            use_count: 0,
            created_by: None,
            last_activity_at: 0,
            created_at: 0,
            cron_referenced: false,
        }
    }
}

/// What a transition pass did.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransitionReport {
    pub checked: usize,
    pub marked_stale: usize,
    pub archived: usize,
    pub reactivated: usize,
}

/// The curator policy. Defaults match the reference: stale after 14 days, archive after 30,
/// interval one week, minimum idle two hours.
#[derive(Debug, Clone)]
pub struct Curator {
    pub interval_hours: i64,
    pub min_idle_hours: i64,
    pub stale_after_days: i64,
    pub archive_after_days: i64,
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    pub paused: bool,
}

impl Default for Curator {
    fn default() -> Self {
        Self {
            interval_hours: 24 * 7,
            min_idle_hours: 2,
            stale_after_days: 14,
            archive_after_days: 30,
            enabled: true,
            last_run_at: None,
            paused: false,
        }
    }
}

impl Curator {
    /// Whether a run is due: enabled, not paused, `last_run_at` present and older than the
    /// interval. A first observation seeds the clock and defers one interval, so a fresh
    /// install never mutates the library on its first tick.
    pub fn should_run(&self, now: i64) -> bool {
        if !self.enabled || self.paused {
            return false;
        }
        match self.last_run_at {
            None => false,
            Some(last) => now - last >= self.interval_hours * 3600,
        }
    }

    /// The model-free pass. Returns what changed. Pinned and cron-referenced skills are exempt.
    pub fn apply_transitions(
        &self,
        records: &mut [SkillRecord],
        now: i64,
    ) -> TransitionReport {
        let stale_cutoff = now - self.stale_after_days * 86_400;
        let archive_cutoff = now - self.archive_after_days * 86_400;
        let mut report = TransitionReport::default();

        for record in records.iter_mut() {
            report.checked += 1;
            if record.pinned || record.cron_referenced {
                continue;
            }
            let anchor = if record.last_activity_at > 0 {
                record.last_activity_at
            } else {
                record.created_at
            };

            // Never archive a never-used skill younger than the stale window.
            if record.use_count == 0 && anchor > stale_cutoff {
                if record.state == SkillState::Stale {
                    record.state = SkillState::Active;
                    report.reactivated += 1;
                }
                continue;
            }

            if anchor <= archive_cutoff && record.state != SkillState::Archived {
                record.state = SkillState::Archived;
                report.archived += 1;
            } else if anchor <= stale_cutoff && record.state == SkillState::Active {
                record.state = SkillState::Stale;
                report.marked_stale += 1;
            } else if anchor > stale_cutoff && record.state == SkillState::Stale {
                record.state = SkillState::Active;
                report.reactivated += 1;
            }
        }
        report
    }
}

/// Learning signal: a skill is graph-worthy when the agent created it, the user taught it,
/// or it has actually been used. External mounts are configured, not learned.
pub fn has_learning_signal(created_by: Option<&str>, use_count: u64) -> bool {
    matches!(created_by, Some("agent") | Some("learn")) || use_count > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use SkillState::{Active, Archived, Stale};

    #[test]
    fn never_used_young_skill_is_not_archived() {
        let curator = Curator::default();
        let now = 1_000_000_000;
        let mut records = vec![SkillRecord {
            created_at: now - 5 * 86_400, // 5 days old
            ..SkillRecord::new("young")
        }];
        let report = curator.apply_transitions(&mut records, now);
        assert_eq!(records[0].state, Active);
        assert_eq!(report.archived, 0);
    }

    #[test]
    fn used_old_skill_ages_to_archived() {
        let curator = Curator::default();
        let now = 1_000_000_000;
        let mut records = vec![SkillRecord {
            use_count: 4,
            last_activity_at: now - 40 * 86_400,
            created_at: now - 60 * 86_400,
            ..SkillRecord::new("old")
        }];
        let report = curator.apply_transitions(&mut records, now);
        assert_eq!(records[0].state, Archived);
        assert_eq!(report.archived, 1);
    }

    #[test]
    fn pinned_and_cron_bypass() {
        let curator = Curator::default();
        let now = 1_000_000_000;
        let mut records = vec![
            SkillRecord {
                pinned: true,
                use_count: 2,
                last_activity_at: now - 90 * 86_400,
                ..SkillRecord::new("pinned")
            },
            SkillRecord {
                cron_referenced: true,
                use_count: 2,
                last_activity_at: now - 90 * 86_400,
                ..SkillRecord::new("cron")
            },
        ];
        curator.apply_transitions(&mut records, now);
        assert_eq!(records[0].state, Active);
        assert_eq!(records[1].state, Active);
    }

    #[test]
    fn reactivates_a_stale_skill_used_again() {
        let curator = Curator::default();
        let now = 1_000_000_000;
        let mut records = vec![SkillRecord {
            state: Stale,
            use_count: 3,
            last_activity_at: now - 86_400,
            ..SkillRecord::new("back")
        }];
        let report = curator.apply_transitions(&mut records, now);
        assert_eq!(records[0].state, Active);
        assert_eq!(report.reactivated, 1);
    }

    #[test]
    fn should_run_gates_on_interval() {
        let curator = Curator {
            last_run_at: Some(0),
            ..Curator::default()
        };
        assert!(!curator.should_run(3600));
        assert!(curator.should_run(24 * 7 * 3600 + 1));
    }
}
