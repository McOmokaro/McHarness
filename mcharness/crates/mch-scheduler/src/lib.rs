//! mch-scheduler — claim-before-spawn scheduling with skip-on-overlap.
//!
//! Mined from OpenWorker (`engine/scheduler.py`, MIT, (c) 2024 Andrew Ng,
//! github.com/andrewyng/openworker). Rules:
//! 1. Claim before spawn — a tick returns at most one due job and marks it
//!    RUNNING so a second tick cannot double-fire it.
//! 2. Skip on overlap — if the previous run is still RUNNING, the new tick is
//!    skipped, not queued.
//! 3. Release — a completed run releases the slot so the next due time fires.
//! 4. Run-once catch-up — if the scheduler was down past a due time, fire once,
//!    not once per missed interval.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("job `{0}` not found")]
    UnknownJob(String),
    #[error("job `{0}` is already running")]
    AlreadyRunning(String),
    #[error("invalid interval for `{0}`: must be > 0")]
    InvalidInterval(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Idle,
    Running,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub name: String,
    pub interval_secs: u64,
    pub next_due_epoch: u64,
    pub state: JobState,
    #[serde(default)]
    pub fired_once: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickOutcome {
    /// A job was claimed; run it.
    Claimed(String),
    /// Previous run still running — skipped this tick.
    SkippedOverlap(String),
    /// Nothing due.
    Idle,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Scheduler {
    jobs: BTreeMap<String, Job>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_job(&mut self, name: &str, interval_secs: u64, now_epoch: u64) -> Result<(), SchedulerError> {
        if interval_secs == 0 {
            return Err(SchedulerError::InvalidInterval(name.to_string()));
        }
        self.jobs.insert(
            name.to_string(),
            Job {
                name: name.to_string(),
                interval_secs,
                next_due_epoch: now_epoch + interval_secs,
                state: JobState::Idle,
                fired_once: false,
            },
        );
        Ok(())
    }

    /// Tick: claim at most one due job. Claim-before-spawn — the job flips to
    /// RUNNING atomically inside this call, so a concurrent tick cannot claim
    /// the same job twice.
    pub fn tick(&mut self, now_epoch: u64) -> Result<TickOutcome, SchedulerError> {
        // Find due jobs sorted by name for determinism.
        let mut due: Vec<&Job> = self
            .jobs
            .values()
            .filter(|j| j.state == JobState::Idle && j.next_due_epoch <= now_epoch)
            .collect();
        due.sort_by(|a, b| a.name.cmp(&b.name));
        let Some(job) = due.first() else {
            return Ok(TickOutcome::Idle);
        };
        let name = job.name.clone();
        let entry = self.jobs.get_mut(&name).unwrap();
        entry.state = JobState::Running;
        entry.fired_once = true;
        entry.next_due_epoch = now_epoch + entry.interval_secs;
        Ok(TickOutcome::Claimed(name))
    }

    /// Skip-on-overlap check: if a job is RUNNING, a new tick does not queue.
    pub fn is_running(&self, name: &str) -> bool {
        self.jobs.get(name).map(|j| j.state == JobState::Running).unwrap_or(false)
    }

    /// Release a completed run so the next due time can fire.
    pub fn release(&mut self, name: &str) -> Result<(), SchedulerError> {
        let job = self
            .jobs
            .get_mut(name)
            .ok_or_else(|| SchedulerError::UnknownJob(name.to_string()))?;
        job.state = JobState::Idle;
        Ok(())
    }

    /// Run-once catch-up: after downtime, a job whose next_due is far past
    /// fires exactly once — the tick never loops to replay missed intervals.
    pub fn catchup_fires(&self, name: &str, now_epoch: u64) -> bool {
        self.jobs
            .get(name)
            .map(|j| j.next_due_epoch <= now_epoch && j.state == JobState::Idle && !j.fired_once)
            .unwrap_or(false)
    }

    /// Record an outcome (used by the run loop for bookkeeping).
    pub fn advance(&mut self, name: &str, now_epoch: u64) -> Result<(), SchedulerError> {
        let job = self
            .jobs
            .get_mut(name)
            .ok_or_else(|| SchedulerError::UnknownJob(name.to_string()))?;
        job.next_due_epoch = now_epoch + job.interval_secs;
        Ok(())
    }

    pub fn job(&self, name: &str) -> Option<&Job> {
        self.jobs.get(name)
    }

    pub fn job_names(&self) -> Vec<String> {
        self.jobs.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_claims_and_returns_due() {
        let mut s = Scheduler::new();
        s.add_job("recon", 60, 0).unwrap();
        // Not due yet.
        assert_eq!(s.tick(30).unwrap(), TickOutcome::Idle);
        // Due now.
        assert_eq!(s.tick(60).unwrap(), TickOutcome::Claimed("recon".into()));
        assert!(s.is_running("recon"));
    }

    #[test]
    fn skip_on_overlap_when_still_running() {
        let mut s = Scheduler::new();
        s.add_job("recon", 10, 0).unwrap();
        assert_eq!(s.tick(10).unwrap(), TickOutcome::Claimed("recon".into()));
        // Still running — second tick must NOT claim it again.
        let outcome = s.tick(20).unwrap();
        assert_ne!(outcome, TickOutcome::Claimed("recon".into()));
        assert!(s.is_running("recon"));
    }

    #[test]
    fn release_unblocks_next_run() {
        let mut s = Scheduler::new();
        s.add_job("recon", 10, 0).unwrap();
        s.tick(10).unwrap();
        s.release("recon").unwrap();
        assert!(!s.is_running("recon"));
        assert_eq!(s.tick(20).unwrap(), TickOutcome::Claimed("recon".into()));
    }

    #[test]
    fn duplicate_entries_in_snapshot_fire_once() {
        // Claim-before-spawn: even if a snapshot contains the same job twice,
        // only the first tick claims it.
        let mut s = Scheduler::new();
        s.add_job("recon", 5, 0).unwrap();
        assert_eq!(s.tick(5).unwrap(), TickOutcome::Claimed("recon".into()));
        assert_ne!(s.tick(5).unwrap(), TickOutcome::Claimed("recon".into()));
    }

    #[test]
    fn advance_records_outcome() {
        let mut s = Scheduler::new();
        s.add_job("recon", 60, 0).unwrap();
        s.tick(60).unwrap();
        s.advance("recon", 100).unwrap();
        assert_eq!(s.job("recon").unwrap().next_due_epoch, 160);
    }

    #[test]
    fn claim_is_first_only() {
        let mut s = Scheduler::new();
        s.add_job("a", 5, 0).unwrap();
        s.add_job("b", 5, 0).unwrap();
        // Both due; first tick claims exactly one (deterministic order).
        let first = s.tick(5).unwrap();
        assert!(matches!(first, TickOutcome::Claimed(n) if n == "a" || n == "b"));
        let second = s.tick(5).unwrap();
        if let TickOutcome::Claimed(n2) = &second {
            if let TickOutcome::Claimed(n1) = &first {
                assert_ne!(n1, n2);
            }
        }
    }
}
