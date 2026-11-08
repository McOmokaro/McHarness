//! Sandbox — Secure execution environment for dynamic testing
//!
//! Forged from Spotlight sandbox and BugBountyFramework harnesses

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Sandbox trait — implemented by concrete sandbox backends (Docker, gVisor, Firecracker).
/// Methods return boxed futures so the trait stays object-safe (`dyn Sandbox`).
pub trait Sandbox: Send + Sync {
    /// Execute a command inside the sandbox
    fn execute<'a>(
        &'a self,
        command: &'a [String],
        timeout: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ExecutionResult, anyhow::Error>> + Send + 'a>>;

    /// Copy a file into the sandbox
    fn copy_in<'a>(
        &'a self,
        src: &'a PathBuf,
        dst: &'a PathBuf,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send + 'a>>;

    /// Copy a file out of the sandbox
    fn copy_out<'a>(
        &'a self,
        src: &'a PathBuf,
        dst: &'a PathBuf,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send + 'a>>;

    /// Query sandbox status
    fn status<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<SandboxStatus, anyhow::Error>> + Send + 'a>>;

    /// Tear down the sandbox and release resources
    fn destroy<'a>(
        &'a mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send + 'a>>;
}

/// Sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Sandbox type
    pub sandbox_type: SandboxType,
    /// Working directory
    pub work_dir: PathBuf,
    /// Network access
    pub network: NetworkConfig,
    /// Resource limits
    pub limits: ResourceLimits,
    /// Mount points
    pub mounts: Vec<Mount>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Allowed egress
    pub egress_allowed: bool,
    /// Egress allowlist
    pub egress_allowlist: Vec<String>,
    /// Timeout
    pub timeout: Duration,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            sandbox_type: SandboxType::Docker,
            work_dir: PathBuf::from("/tmp/sandbox"),
            network: NetworkConfig::default(),
            limits: ResourceLimits::default(),
            mounts: Vec::new(),
            env: HashMap::new(),
            egress_allowed: false,
            egress_allowlist: Vec::new(),
            timeout: Duration::from_secs(300),
        }
    }
}

/// Sandbox types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxType {
    /// Docker container
    Docker,
    /// gVisor sandbox
    GVisor,
    /// Firecracker microVM
    Firecracker,
    /// Native (no sandbox - for trusted code only)
    Native,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Enable network
    pub enabled: bool,
    /// Proxy URL
    pub proxy: Option<String>,
    /// Allowed domains
    pub allowed_domains: Vec<String>,
    /// Blocked domains
    pub blocked_domains: Vec<String>,
    /// Port forwarding
    pub port_forwarding: Vec<PortForward>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            proxy: None,
            allowed_domains: Vec::new(),
            blocked_domains: Vec::new(),
            port_forwarding: Vec::new(),
        }
    }
}

/// Port forwarding rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForward {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
}

/// Resource limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU cores
    pub cpu_cores: f32,
    /// Memory (MB)
    pub memory_mb: u64,
    /// Disk (MB)
    pub disk_mb: u64,
    /// Max processes
    pub max_processes: u32,
    /// Max file descriptors
    pub max_fds: u32,
    /// Max execution time (seconds)
    pub max_time_secs: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_cores: 1.0,
            memory_mb: 512,
            disk_mb: 1024,
            max_processes: 100,
            max_fds: 1024,
            max_time_secs: 300,
        }
    }
}

/// Mount point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mount {
    pub source: PathBuf,
    pub target: PathBuf,
    pub read_only: bool,
    pub mount_type: MountType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MountType {
    Bind,
    Tmpfs,
    Volume,
}

/// Capability token for sandbox operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// Agent role
    pub agent_role: String,
    /// Finding ID this token is for
    pub finding_id: Option<String>,
    /// Read-only paths
    pub ro_paths: Vec<String>,
    /// Read-write paths
    pub rw_paths: Vec<String>,
    /// Egress allowed
    pub egress_allowed: bool,
    /// Egress allowlist
    pub egress_allowlist: Vec<String>,
    /// CPU limit
    pub cpu: f32,
    /// Memory (MB)
    pub memory_mb: u64,
    /// Timeout (seconds)
    pub timeout_secs: u64,
    /// Max execution calls
    pub max_exec_calls: u32,
    /// Max output bytes
    pub max_output_bytes: u64,
    /// Token ID for audit trail
    #[serde(skip)]
    pub token_id: Option<String>,
}

/// Capability roles
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRole {
    Reproducer,
    Verifier,
    Scanner,
    Analyzer,
    Custom(String),
}

impl CapabilityToken {
    pub fn for_reproducer(finding_id: String, repo_path: String) -> Self {
        Self {
            agent_role: "reproducer".to_string(),
            finding_id: Some(finding_id),
            ro_paths: vec![repo_path.clone()],
            rw_paths: vec![format!("{}/patched", repo_path)],
            egress_allowed: false,
            egress_allowlist: Vec::new(),
            cpu: 1.0,
            memory_mb: 512,
            timeout_secs: 300,
            max_exec_calls: 10,
            max_output_bytes: 512 * 1024,
            token_id: None,
        }
    }

    pub fn for_verifier(finding_id: String, patched_path: String) -> Self {
        Self {
            agent_role: "verifier".to_string(),
            finding_id: Some(finding_id),
            ro_paths: vec![patched_path.clone()],
            rw_paths: Vec::new(),
            egress_allowed: false,
            egress_allowlist: Vec::new(),
            cpu: 1.0,
            memory_mb: 512,
            timeout_secs: 300,
            max_exec_calls: 10,
            max_output_bytes: 512 * 1024,
            token_id: None,
        }
    }
}

/// Sandbox execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub oom_killed: bool,
}

/// Sandbox status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxStatus {
    pub running: bool,
    pub cpu_usage: f32,
    pub memory_usage_mb: u64,
    pub disk_usage_mb: u64,
    pub uptime_secs: u64,
}

/// Docker sandbox implementation
pub struct DockerSandbox {
    config: SandboxConfig,
    container_id: Option<String>,
}

impl DockerSandbox {
    pub fn new(config: SandboxConfig) -> Self {
        Self {
            config,
            container_id: None,
        }
    }

    /// Execute a command in the sandbox
    pub async fn execute(&self, command: &[String], timeout: Duration) -> Result<ExecutionResult, anyhow::Error> {
        // In real implementation, would use bollard or docker CLI
        Ok(ExecutionResult {
            success: true,
            stdout: "Mock output".to_string(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 100,
            timed_out: false,
            oom_killed: false,
        })
    }

    /// Copy file into sandbox
    pub async fn copy_in(&self, _src: &PathBuf, _dst: &PathBuf) -> Result<(), anyhow::Error> {
        Ok(())
    }

    /// Copy file out of sandbox
    pub async fn copy_out(&self, _src: &PathBuf, _dst: &PathBuf) -> Result<(), anyhow::Error> {
        Ok(())
    }

    /// Get sandbox status
    pub async fn status(&self) -> Result<SandboxStatus, anyhow::Error> {
        Ok(SandboxStatus {
            running: self.container_id.is_some(),
            cpu_usage: 0.0,
            memory_usage_mb: 0,
            disk_usage_mb: 0,
            uptime_secs: 0,
        })
    }

    /// Destroy sandbox
    pub async fn destroy(&mut self) -> Result<(), anyhow::Error> {
        self.container_id = None;
        Ok(())
    }
}

impl Sandbox for DockerSandbox {
    fn execute<'a>(
        &'a self,
        command: &'a [String],
        timeout: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ExecutionResult, anyhow::Error>> + Send + 'a>> {
        Box::pin(async move { DockerSandbox::execute(self, command, timeout).await })
    }

    fn copy_in<'a>(
        &'a self,
        src: &'a PathBuf,
        dst: &'a PathBuf,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send + 'a>> {
        Box::pin(async move { DockerSandbox::copy_in(self, src, dst).await })
    }

    fn copy_out<'a>(
        &'a self,
        src: &'a PathBuf,
        dst: &'a PathBuf,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send + 'a>> {
        Box::pin(async move { DockerSandbox::copy_out(self, src, dst).await })
    }

    fn status<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<SandboxStatus, anyhow::Error>> + Send + 'a>> {
        Box::pin(async move { DockerSandbox::status(self).await })
    }

    fn destroy<'a>(
        &'a mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send + 'a>> {
        Box::pin(async move { DockerSandbox::destroy(self).await })
    }
}

/// Factory for creating sandboxes
pub struct SandboxFactory;

impl SandboxFactory {
    pub fn create(config: SandboxConfig) -> Result<DockerSandbox, anyhow::Error> {
        match config.sandbox_type {
            SandboxType::Docker => Ok(DockerSandbox::new(config)),
            SandboxType::GVisor => Err(anyhow::anyhow!("GVisor not implemented")),
            SandboxType::Firecracker => Err(anyhow::anyhow!("Firecracker not implemented")),
            SandboxType::Native => Err(anyhow::anyhow!("Native sandbox not implemented")),
        }
    }
}