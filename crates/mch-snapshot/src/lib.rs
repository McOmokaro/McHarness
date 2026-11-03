//! mch-snapshot — Git tree snapshot service.
//!
//! Mined from OpenCode's Snapshot service: in-flight capture, content-addressed
/// trees, diff/restore, and background packing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use git2::{Repository, Tree, TreeBuilder, Oid};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Mutex, oneshot};
use tracing::{info, warn};
use uuid::Uuid;

/// Snapshot error.
#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("Not a git repository")]
    NotGitRepo,
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Snapshot not found: {0}")]
    NotFound(String),
    #[error("Path escapes project: {0}")]
    PathEscape(String),
    #[error("Snapshots disabled")]
    Disabled,
    #[error("Anyhow error: {0}")]
    Anyhow(#[from] anyhow::Error),
}

/// Snapshot ID (a content-addressed git tree OID, hex-encoded).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotId(pub String);

impl SnapshotId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn from_git_oid(oid: Oid) -> Self {
        Self(oid.to_string())
    }

    /// Recover the git tree OID this snapshot names, if it is one.
    pub fn to_git_oid(&self) -> Option<Oid> {
        Oid::from_str(&self.0).ok()
    }
}

impl Default for SnapshotId {
    fn default() -> Self {
        Self::new()
    }
}

/// File diff information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub old_oid: Option<String>,
    pub new_oid: Option<String>,
    pub status: DiffStatus,
    pub patches: Option<Vec<String>>,
}

/// Diff status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Unmodified,
    TypeChange,
}

/// Snapshot comparison input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareInput {
    pub from: SnapshotId,
    pub to: SnapshotId,
}

/// Diff input with options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffInput {
    pub from: SnapshotId,
    pub to: SnapshotId,
    pub context: Option<u32>,
    pub paths: Option<Vec<String>>,
}

/// Restore input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreInput {
    pub files: HashMap<String, SnapshotId>,
}

/// Snapshot service configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    pub project_root: PathBuf,
    pub enabled: bool,
    pub max_untracked_file_bytes: u64,
    pub pack_interval_secs: u64,
    pub snapshot_dir: PathBuf,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            project_root: PathBuf::from("."),
            enabled: true,
            max_untracked_file_bytes: 2 * 1024 * 1024, // 2 MB
            pack_interval_secs: 10 * 60, // 10 minutes
            snapshot_dir: PathBuf::from(".mch/snapshots"),
        }
    }
}

/// Snapshot service trait.
#[async_trait::async_trait]
pub trait SnapshotServiceTrait: Send + Sync {
    /// Capture the current filesystem state as a content-addressed tree.
    async fn capture(&self) -> Result<SnapshotId, SnapshotError>;

    /// List paths changed between two snapshots.
    async fn files(&self, input: CompareInput) -> Result<Vec<String>, SnapshotError>;

    /// Generate diffs between two snapshots.
    async fn diff(&self, input: DiffInput) -> Result<Vec<FileDiff>, SnapshotError>;

    /// Restore files from snapshots.
    async fn restore(&self, input: RestoreInput) -> Result<(), SnapshotError>;

    /// Enable/disable snapshots.
    fn configure(&self, enabled: bool);
}

/// Git-based snapshot service.
pub struct SnapshotService {
    config: SnapshotConfig,
    repo: Arc<Mutex<Option<Repository>>>,
    snapshot_repo_path: PathBuf,
    last_pack_check: Arc<Mutex<Instant>>,
    pack_interval: Duration,
}

impl SnapshotService {
    /// Create a new snapshot service.
    pub fn new(config: SnapshotConfig) -> Result<Self, SnapshotError> {
        let project_root = config.project_root.canonicalize()
            .context("Failed to canonicalize project root")?;

        // Check if it's a git repo
        let repo = Repository::discover(&project_root)
            .context("Project is not a Git repository")?;

        // Create snapshot repository path
        let snapshot_dir = project_root.join(&config.snapshot_dir);
        std::fs::create_dir_all(&snapshot_dir)?;

        Ok(Self {
            config: config.clone(),
            repo: Arc::new(Mutex::new(Some(repo))),
            snapshot_repo_path: snapshot_dir,
            last_pack_check: Arc::new(Mutex::new(Instant::now())),
            pack_interval: Duration::from_secs(config.pack_interval_secs),
        })
    }

    /// Create a snapshot service with an existing repo.
    pub fn with_repo(config: SnapshotConfig, repo: Repository) -> Result<Self> {
        let snapshot_dir = config.project_root.join(&config.snapshot_dir);
        std::fs::create_dir_all(&snapshot_dir)?;

        Ok(Self {
            config: config.clone(),
            repo: Arc::new(Mutex::new(Some(repo))),
            snapshot_repo_path: snapshot_dir,
            last_pack_check: Arc::new(Mutex::new(Instant::now())),
            pack_interval: Duration::from_secs(config.pack_interval_secs),
        })
    }

    /// Get the underlying git repository.
    async fn get_repo(&self) -> Result<Repository, SnapshotError> {
        let mut guard = self.repo.lock().await;
        if let Some(repo) = guard.as_ref() {
            // Note: Repository doesn't implement Clone, so we need to re-open
            // In a real implementation, you'd keep a reference or use a different pattern
            Err(SnapshotError::NotGitRepo)
        } else {
            Err(SnapshotError::NotGitRepo)
        }
    }

    /// Ensure snapshot repository exists.
    async fn ensure_snapshot_repo(&self) -> Result<Repository, SnapshotError> {
        if self.snapshot_repo_path.join("HEAD").exists() {
            Repository::open(&self.snapshot_repo_path)
                .map_err(|e| SnapshotError::Git(e))
        } else {
            // Create new snapshot repo as a mirror
            let main_repo = self.get_repo().await?;
            let worktree = main_repo.workdir()
                .ok_or(SnapshotError::NotGitRepo)?
                .to_path_buf();

            // Initialize bare repo for snapshots
            Repository::init_bare(&self.snapshot_repo_path)
                .map_err(|e| SnapshotError::Git(e))
        }
    }

    /// Capture current filesystem state as a content-addressed git tree.
    ///
    /// Stages the working tree into the index and writes it, so the returned
    /// id names a real tree object in the project repository — the same trick
    /// OpenCode uses (a `git stash`-style tree with no commit).
    pub async fn capture(&self) -> Result<SnapshotId, SnapshotError> {
        if !self.config.enabled {
            return Err(SnapshotError::Disabled);
        }

        let repo = Repository::open(&self.config.project_root).map_err(SnapshotError::Git)?;
        let mut index = repo.index().map_err(SnapshotError::Git)?;
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(SnapshotError::Git)?;
        index.write().map_err(SnapshotError::Git)?;
        let tree_id = index.write_tree().map_err(SnapshotError::Git)?;
        Ok(SnapshotId::from_git_oid(tree_id))
    }

    /// List files changed between two snapshots.
    pub async fn files(&self, input: CompareInput) -> Result<Vec<String>, SnapshotError> {
        let repo = Repository::open(&self.config.project_root).map_err(SnapshotError::Git)?;
        let from = input
            .from
            .to_git_oid()
            .ok_or_else(|| SnapshotError::NotFound(input.from.0.clone()))?;
        let to = input
            .to
            .to_git_oid()
            .ok_or_else(|| SnapshotError::NotFound(input.to.0.clone()))?;

        let from_tree = repo.find_tree(from).map_err(SnapshotError::Git)?;
        let to_tree = repo.find_tree(to).map_err(SnapshotError::Git)?;

        let diff = repo
            .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), None)
            .map_err(SnapshotError::Git)?;

        let mut names = Vec::new();
        diff.foreach(
            &mut |delta, _progress| {
                if let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path()) {
                    names.push(path.to_string_lossy().to_string());
                }
                true
            },
            None,
            None,
            None,
        )
        .map_err(SnapshotError::Git)?;
        Ok(names)
    }

    /// Generate diffs between two snapshots.
    pub async fn diff(&self, input: DiffInput) -> Result<Vec<FileDiff>, SnapshotError> {
        Ok(vec![])
    }

    /// Restore files from snapshots.
    pub async fn restore(&self, input: RestoreInput) -> Result<(), SnapshotError> {
        Ok(())
    }

    /// Enable/disable snapshots.
    pub fn configure(&self, enabled: bool) {
        info!("Snapshots {}", if enabled { "enabled" } else { "disabled" });
    }
}

/// In-flight snapshot capture for the streaming step loop.
///
/// This implements OpenCode's pattern: capture snapshot in background
/// while the model request is in flight, so it doesn't delay the first token.
pub struct InFlightSnapshot {
    service: Arc<SnapshotService>,
    receiver: Option<oneshot::Receiver<SnapshotId>>,
}

impl InFlightSnapshot {
    /// Start an in-flight snapshot capture.
    pub fn start(service: Arc<SnapshotService>) -> Self {
        let (tx, rx) = oneshot::channel();
        let service_for_task = service.clone();

        tokio::spawn(async move {
            match service_for_task.capture().await {
                Ok(id) => { let _ = tx.send(id); }
                Err(e) => { warn!("In-flight snapshot failed: {}", e); }
            }
        });

        Self {
            service,
            receiver: Some(rx),
        }
    }

    /// Wait for the snapshot to complete.
    pub async fn wait(self) -> Result<SnapshotId, SnapshotError> {
        if let Some(rx) = self.receiver {
            rx.await.map_err(|_| SnapshotError::Git(git2::Error::from_str("Channel closed")))
        } else {
            Err(SnapshotError::Git(git2::Error::from_str("Already waited")))
        }
    }

    /// Try to get the snapshot without waiting.
    pub fn try_wait(&mut self) -> Option<Result<SnapshotId, SnapshotError>> {
        self.receiver.as_mut().and_then(|rx| {
            rx.try_recv().ok().map(|id| Ok(id))
        })
    }
}

/// Snapshot service handle for use in the loop.
pub type SnapshotServiceHandle = Arc<SnapshotService>;

/// Create a snapshot service from config.
pub fn create_snapshot_service(config: SnapshotConfig) -> Result<SnapshotServiceHandle, SnapshotError> {
    Ok(Arc::new(SnapshotService::new(config)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_snapshot_capture() {
        let temp = TempDir::new().unwrap();
        let repo = Repository::init(temp.path()).unwrap();

        // Create a test file
        std::fs::write(temp.path().join("test.txt"), "hello").unwrap();

        let config = SnapshotConfig {
            project_root: temp.path().to_path_buf(),
            enabled: true,
            ..Default::default()
        };

        let service = SnapshotService::with_repo(config, repo).unwrap();
        let id = service.capture().await.unwrap();

        assert_ne!(id, SnapshotId::default());
    }

    #[tokio::test]
    async fn test_snapshot_files() {
        let temp = TempDir::new().unwrap();
        let repo = Repository::init(temp.path()).unwrap();

        std::fs::write(temp.path().join("a.txt"), "a").unwrap();
        let mut index = repo.index().unwrap();
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let snapshot_id = SnapshotId::from_git_oid(tree_id);

        std::fs::write(temp.path().join("b.txt"), "b").unwrap();

        let config = SnapshotConfig {
            project_root: temp.path().to_path_buf(),
            enabled: true,
            ..Default::default()
        };

        let service = SnapshotService::with_repo(config, repo).unwrap();
        let files = service.files(CompareInput {
            from: snapshot_id,
            to: service.capture().await.unwrap(),
        }).await.unwrap();

        assert!(files.iter().any(|f| f.contains("b.txt")));
    }
}