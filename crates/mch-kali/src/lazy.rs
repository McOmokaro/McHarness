//! Omarchy lazy-loading engine.
//!
//! Tools are *placeholders* until first use. When the agent requests a tool,
//! the loader checks whether the binary exists; if not, it installs it on
//! demand (via `apt-get` on Kali, or a single-tool Docker image) and then
//! unloads/cleans when done to stay featherweight.

use crate::tools::KaliTool;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

/// How the loader should materialize a missing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadStrategy {
    /// Use the binary if present; never install.
    ExistingOnly,
    /// `apt-get install -y <pkg>` on demand (Kali/Debian).
    AptGet,
    /// `docker run --rm <image> <tool>` (single-tool container).
    Docker,
}

impl Default for LoadStrategy {
    fn default() -> Self {
        LoadStrategy::AptGet
    }
}

/// Lazy-loader configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LazyLoadConfig {
    /// Strategy used when a tool is missing.
    pub strategy: LoadStrategy,
    /// Docker image to use for the `Docker` strategy.
    pub docker_image: String,
    /// apt-get timeout
    pub install_timeout_secs: u64,
    /// Search paths to probe for binaries.
    pub search_paths: Vec<PathBuf>,
    /// If true, only dry-run installs (report what *would* happen).
    pub dry_run: bool,
}

impl Default for LazyLoadConfig {
    fn default() -> Self {
        Self {
            strategy: LoadStrategy::AptGet,
            docker_image: "kalilinux/kali-rolling".to_string(),
            install_timeout_secs: 600,
            search_paths: vec![
                PathBuf::from("/usr/bin"),
                PathBuf::from("/usr/local/bin"),
                PathBuf::from("/usr/sbin"),
                PathBuf::from("/opt"),
            ],
            dry_run: false,
        }
    }
}

/// The lazy loader. Tracks which tools have been materialized so we can clean
/// them up after a campaign.
pub struct LazyLoader {
    config: LazyLoadConfig,
    loaded: std::sync::Mutex<Vec<String>>,
}

impl LazyLoader {
    pub fn new(config: LazyLoadConfig) -> Self {
        Self {
            config,
            loaded: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Locate a binary across the configured search paths (and `PATH`).
    pub fn locate(&self, binary: &str) -> Option<PathBuf> {
        // Respect PATH first.
        if let Ok(path) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path) {
                let candidate = dir.join(binary);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        for dir in &self.config.search_paths {
            let candidate = dir.join(binary);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Ensure a tool is available. Returns the resolved binary path.
    ///
    /// Never fails hard on a missing install when `dry_run` is set: it returns
    /// `Ok(None)` so callers can continue in a "planned" mode.
    pub async fn ensure(&self, tool: KaliTool) -> anyhow::Result<Option<PathBuf>> {
        if let Some(path) = self.locate(tool.binary()) {
            return Ok(Some(path));
        }

        match self.config.strategy {
            LoadStrategy::ExistingOnly => Ok(None),
            LoadStrategy::AptGet => {
                if self.config.dry_run {
                    info!("[dry-run] would apt-get install {}", tool.apt_package());
                    return Ok(None);
                }
                self.apt_install(tool).await?;
                Ok(self.locate(tool.binary()))
            }
            LoadStrategy::Docker => {
                // Docker path: the binary is invoked via `docker run`; there is
                // no local path to resolve.
                info!(
                    "tool {} will be executed via docker image {}",
                    tool.binary(),
                    self.config.docker_image
                );
                Ok(None)
            }
        }
    }

    /// Install via apt-get.
    async fn apt_install(&self, tool: KaliTool) -> anyhow::Result<()> {
        let pkg = tool.apt_package();
        info!("lazy-loading {} via apt-get (package {})", tool.binary(), pkg);
        let status = tokio::time::timeout(
            Duration::from_secs(self.config.install_timeout_secs),
            tokio::process::Command::new("apt-get")
                .arg("install")
                .arg("-y")
                .arg(pkg)
                .status(),
        )
        .await;

        match status {
            Ok(Ok(s)) if s.success() => {
                self.loaded.lock().unwrap().push(tool.binary().to_string());
                Ok(())
            }
            Ok(Ok(s)) => Err(anyhow::anyhow!(
                "apt-get install {pkg} exited with status {s}"
            )),
            Ok(Err(e)) => Err(anyhow::anyhow!("apt-get spawn failed: {e}")),
            Err(_) => Err(anyhow::anyhow!("apt-get install {pkg} timed out")),
        }
    }

    /// Report whether a tool is currently materialized.
    pub fn is_available(&self, tool: KaliTool) -> bool {
        self.locate(tool.binary()).is_some()
    }

    /// Tools materialized during this campaign.
    pub fn loaded_tools(&self) -> Vec<String> {
        self.loaded.lock().unwrap().clone()
    }

    /// Unload/cleanup: remove only the tools we installed (best-effort).
    pub async fn cleanup(&self) -> anyhow::Result<()> {
        let loaded = self.loaded.lock().unwrap().clone();
        for bin in &loaded {
            if let Some(tool) = KaliTool::from_binary(bin) {
                warn!("unloading {}", tool.binary());
                let _ = tokio::process::Command::new("apt-get")
                    .arg("remove")
                    .arg("-y")
                    .arg(tool.apt_package())
                    .status()
                    .await;
            }
        }
        self.loaded.lock().unwrap().clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let c = LazyLoadConfig::default();
        assert_eq!(c.strategy, LoadStrategy::AptGet);
        assert!(!c.dry_run);
    }

    #[test]
    fn locate_finds_a_real_binary() {
        // `sh` is virtually always present.
        let loader = LazyLoader::new(LazyLoadConfig::default());
        assert!(loader.locate("sh").is_some());
    }
}
