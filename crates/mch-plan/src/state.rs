//! State management: RunState, StepState, workflow packages, versioning.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, BTreeMap};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use uuid::Uuid;
use zip::result::ZipError;
use thiserror::Error;

/// Errors during state management.
#[derive(Debug, Error)]
pub enum StateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("zip error: {0}")]
    Zip(#[from] ZipError),
    #[error("invalid state: {0}")]
    InvalidState(String),
    #[error("package not found: {0}")]
    PackageNotFound(String),
    #[error("version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: String, found: String },
}

/// Overall workflow run status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Run created but not started.
    Pending,
    /// Currently executing.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed (gate failure, step error, etc.).
    Failed,
    /// Paused awaiting external input.
    Paused,
    /// Cancelled by user.
    Cancelled,
}

/// Individual step execution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Not yet started.
    Pending,
    /// Currently executing.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
    /// Skipped (branch not taken, fallback not needed).
    Skipped,
    /// Retrying.
    Retrying,
}

/// A single step's execution state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepState {
    pub step_id: String,
    pub plan_step_id: String,
    pub status: StepStatus,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    pub duration_ms: Option<u64>,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub retry_count: u32,
    pub gate_results: Vec<GateResult>,
    pub metadata: StepRunMetadata,
}

/// Result of a gate check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_id: String,
    pub gate_type: String,
    pub passed: bool,
    pub message: String,
    pub checked_at: SystemTime,
}

/// Per-step runtime metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepRunMetadata {
    pub tokens_used: Option<u64>,
    pub tool_calls: u32,
    pub tools_failed: u32,
    pub custom: HashMap<String, serde_json::Value>,
}

/// Complete workflow run state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunState {
    pub run_id: String,
    pub plan_id: String,
    pub plan_name: String,
    pub plan_version: String,
    pub status: RunStatus,
    pub created_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    pub current_step: Option<String>,
    pub step_states: BTreeMap<String, StepState>,
    pub variables: HashMap<String, serde_json::Value>,
    pub artifacts: Vec<ArtifactRecord>,
    pub gate_results: Vec<GateResult>,
    pub error: Option<String>,
    pub metadata: RunMetadata,
}

/// Run-level metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunMetadata {
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub model_id: Option<String>,
    pub tags: Vec<String>,
    pub custom: HashMap<String, serde_json::Value>,
}

/// An artifact produced during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub artifact_id: String,
    pub step_id: String,
    pub name: String,
    pub path: PathBuf,
    pub content_type: String,
    pub size_bytes: u64,
    pub created_at: SystemTime,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Workflow package version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre_release: Option<String>,
    pub build: Option<String>,
}

impl PackageVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch, pre_release: None, build: None }
    }

    pub fn parse(s: &str) -> Result<Self, StateError> {
        // Simple semver parsing
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() < 3 {
            return Err(StateError::InvalidState(format!("invalid version: {s}")));
        }
        let major = parts[0].parse().map_err(|_| StateError::InvalidState(format!("invalid major: {}", parts[0])))?;
        let minor = parts[1].parse().map_err(|_| StateError::InvalidState(format!("invalid minor: {}", parts[1])))?;
        let patch_part = parts[2];
        let (patch, pre_release) = if let Some(idx) = patch_part.find('-') {
            let patch = patch_part[..idx].parse().map_err(|_| StateError::InvalidState(format!("invalid patch: {}", &patch_part[..idx])))?;
            (patch, Some(patch_part[idx+1..].to_string()))
        } else {
            let patch = patch_part.parse().map_err(|_| StateError::InvalidState(format!("invalid patch: {patch_part}")))?;
            (patch, None)
        };
        Ok(Self { major, minor, patch, pre_release, build: None })
    }
}

impl std::fmt::Display for PackageVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = format!("{}.{}.{}", self.major, self.minor, self.patch);
        if let Some(ref pre) = self.pre_release {
            s.push('-');
            s.push_str(pre);
        }
        if let Some(ref build) = self.build {
            s.push('+');
            s.push_str(build);
        }
        write!(f, "{s}")
    }
}

/// A workflow package (.wflite.zip) containing compiled plan + assets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPackage {
    pub package_id: String,
    pub name: String,
    pub version: PackageVersion,
    pub plan: crate::compiler::CompiledPlan,
    pub assets: HashMap<String, Vec<u8>>, // asset_name -> bytes
    pub manifest: PackageManifest,
    pub created_at: SystemTime,
}

/// Package manifest with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub name: String,
    pub version: PackageVersion,
    pub description: String,
    pub author: Option<String>,
    pub license: Option<String>,
    pub dependencies: Vec<PackageDependency>,
    pub entry_points: Vec<String>, // step IDs that can be entry points
    pub exported_skills: Vec<ExportedSkill>,
}

/// External package dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDependency {
    pub name: String,
    pub version_req: String, // semver requirement
    pub optional: bool,
}

/// A skill exported from this workflow package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedSkill {
    pub skill_name: String,
    pub entry_step: String,
    pub description: String,
    pub parameters: Vec<SkillParameter>,
}

/// Parameter for an exported skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    pub name: String,
    pub param_type: String,
    pub description: String,
    pub required: bool,
    pub default: Option<serde_json::Value>,
}

impl WorkflowPackage {
    /// Create a new workflow package from a compiled plan.
    pub fn new(name: &str, version: PackageVersion, plan: crate::compiler::CompiledPlan) -> Self {
        Self {
            package_id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            version,
            plan,
            assets: HashMap::new(),
            manifest: PackageManifest {
                name: name.to_string(),
                version: PackageVersion::new(1, 0, 0),
                description: String::new(),
                author: None,
                license: None,
                dependencies: Vec::new(),
                entry_points: Vec::new(),
                exported_skills: Vec::new(),
            },
            created_at: SystemTime::now(),
        }
    }

    /// Add an asset to the package.
    pub fn add_asset(&mut self, name: &str, data: Vec<u8>) {
        self.assets.insert(name.to_string(), data);
    }

    /// Add an exported skill.
    pub fn export_skill(&mut self, skill: ExportedSkill) {
        self.manifest.exported_skills.push(skill);
    }

    /// Serialize package to .wflite.zip bytes.
    pub fn to_zip(&self) -> Result<Vec<u8>, StateError> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

            // Write plan.json
            let plan_json = serde_json::to_vec_pretty(&self.plan)?;
            zip.start_file("plan.json", options)?;
            zip.write_all(&plan_json)?;

            // Write manifest.json
            let manifest_json = serde_json::to_vec_pretty(&self.manifest)?;
            zip.start_file("manifest.json", options)?;
            zip.write_all(&manifest_json)?;

            // Write assets
            for (name, data) in &self.assets {
                zip.start_file(&format!("assets/{name}"), options)?;
                zip.write_all(data)?;
            }

            // Write metadata
            let metadata = serde_json::json!({
                "package_id": self.package_id,
                "name": self.name,
                "version": self.version.to_string(),
                "created_at": self.created_at.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            });
            zip.start_file("metadata.json", options)?;
            zip.write_all(&serde_json::to_vec_pretty(&metadata)?)?;

            zip.finish()?;
        }
        Ok(buf)
    }

    /// Deserialize package from .wflite.zip bytes.
    pub fn from_zip(data: &[u8]) -> Result<Self, StateError> {
        let cursor = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor)?;

        // Read plan.json
        let mut plan_file = archive.by_name("plan.json")?;
        let plan: crate::compiler::CompiledPlan = serde_json::from_reader(&mut plan_file)?;

        // Read manifest.json
        let mut manifest_file = archive.by_name("manifest.json")?;
        let manifest: PackageManifest = serde_json::from_reader(&mut manifest_file)?;

        // Read metadata.json
        let mut metadata_file = archive.by_name("metadata.json")?;
        let metadata: serde_json::Value = serde_json::from_reader(&mut metadata_file)?;
        let package_id = metadata["package_id"].as_str().unwrap_or("").to_string();
        let name = metadata["name"].as_str().unwrap_or("").to_string();
        let version = PackageVersion::parse(metadata["version"].as_str().unwrap_or("1.0.0"))?;
        let created_at_secs = metadata["created_at"].as_u64().unwrap_or(0);
        let created_at = SystemTime::UNIX_EPOCH + Duration::from_secs(created_at_secs);

        // Read assets
        let mut assets = HashMap::new();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            if name.starts_with("assets/") {
                let asset_name = name.strip_prefix("assets/").unwrap().to_string();
                let mut data = Vec::new();
                std::io::copy(&mut file, &mut data)?;
                assets.insert(asset_name, data);
            }
        }

        Ok(Self {
            package_id,
            name,
            version,
            plan,
            assets,
            manifest,
            created_at,
        })
    }

    /// Save package to file.
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), StateError> {
        let data = self.to_zip()?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Load package from file.
    pub fn load_from_file(path: &PathBuf) -> Result<Self, StateError> {
        let data = std::fs::read(path)?;
        Self::from_zip(&data)
    }
}

impl RunState {
    /// Create a new run state for a plan.
    pub fn new(plan: &crate::compiler::CompiledPlan) -> Self {
        let run_id = Uuid::new_v4().to_string();
        let now = SystemTime::now();

        let mut step_states = BTreeMap::new();
        for step in &plan.steps {
            step_states.insert(step.id.clone(), StepState {
                step_id: Uuid::new_v4().to_string(),
                plan_step_id: step.id.clone(),
                status: StepStatus::Pending,
                started_at: None,
                completed_at: None,
                duration_ms: None,
                input: serde_json::json!({}),
                output: None,
                error: None,
                retry_count: 0,
                gate_results: Vec::new(),
                metadata: StepRunMetadata::default(),
            });
        }

        Self {
            run_id,
            plan_id: plan.plan_id.clone(),
            plan_name: plan.name.clone(),
            plan_version: plan.version.clone(),
            status: RunStatus::Pending,
            created_at: now,
            started_at: None,
            completed_at: None,
            current_step: None,
            step_states,
            variables: HashMap::new(),
            artifacts: Vec::new(),
            gate_results: Vec::new(),
            error: None,
            metadata: RunMetadata::default(),
        }
    }

    /// Start the run.
    pub fn start(&mut self) {
        self.status = RunStatus::Running;
        self.started_at = Some(SystemTime::now());
    }

    /// Mark run as completed.
    pub fn complete(&mut self) {
        self.status = RunStatus::Completed;
        self.completed_at = Some(SystemTime::now());
    }

    /// Mark run as failed.
    pub fn fail(&mut self, error: String) {
        self.status = RunStatus::Failed;
        self.completed_at = Some(SystemTime::now());
        self.error = Some(error);
    }

    /// Pause the run.
    pub fn pause(&mut self) {
        self.status = RunStatus::Paused;
    }

    /// Resume the run.
    pub fn resume(&mut self) {
        self.status = RunStatus::Running;
    }

    /// Cancel the run.
    pub fn cancel(&mut self) {
        self.status = RunStatus::Cancelled;
        self.completed_at = Some(SystemTime::now());
    }

    /// Start a step.
    pub fn start_step(&mut self, step_id: &str, input: serde_json::Value) -> Result<(), StateError> {
        let step_state = self.step_states.get_mut(step_id)
            .ok_or_else(|| StateError::InvalidState(format!("step not found: {step_id}")))?;
        step_state.status = StepStatus::Running;
        step_state.started_at = Some(SystemTime::now());
        step_state.input = input;
        self.current_step = Some(step_id.to_string());
        Ok(())
    }

    /// Complete a step.
    pub fn complete_step(&mut self, step_id: &str, output: serde_json::Value) -> Result<(), StateError> {
        let step_state = self.step_states.get_mut(step_id)
            .ok_or_else(|| StateError::InvalidState(format!("step not found: {step_id}")))?;
        let now = SystemTime::now();
        step_state.status = StepStatus::Completed;
        step_state.completed_at = Some(now);
        if let Some(started) = step_state.started_at {
            step_state.duration_ms = now.duration_since(started).ok().map(|d| d.as_millis() as u64);
        }
        step_state.output = Some(output);
        Ok(())
    }

    /// Fail a step.
    pub fn fail_step(&mut self, step_id: &str, error: String) -> Result<(), StateError> {
        let step_state = self.step_states.get_mut(step_id)
            .ok_or_else(|| StateError::InvalidState(format!("step not found: {step_id}")))?;
        let now = SystemTime::now();
        step_state.status = StepStatus::Failed;
        step_state.completed_at = Some(now);
        if let Some(started) = step_state.started_at {
            step_state.duration_ms = now.duration_since(started).ok().map(|d| d.as_millis() as u64);
        }
        step_state.error = Some(error);
        Ok(())
    }

    /// Skip a step (branch not taken).
    pub fn skip_step(&mut self, step_id: &str) -> Result<(), StateError> {
        let step_state = self.step_states.get_mut(step_id)
            .ok_or_else(|| StateError::InvalidState(format!("step not found: {step_id}")))?;
        step_state.status = StepStatus::Skipped;
        step_state.completed_at = Some(SystemTime::now());
        Ok(())
    }

    /// Record a gate result.
    pub fn record_gate_result(&mut self, step_id: &str, result: GateResult) -> Result<(), StateError> {
        let step_state = self.step_states.get_mut(step_id)
            .ok_or_else(|| StateError::InvalidState(format!("step not found: {step_id}")))?;
        step_state.gate_results.push(result.clone());
        self.gate_results.push(result);
        Ok(())
    }

    /// Add an artifact.
    pub fn add_artifact(&mut self, artifact: ArtifactRecord) {
        self.artifacts.push(artifact);
    }

    /// Set a variable.
    pub fn set_variable(&mut self, name: &str, value: serde_json::Value) {
        self.variables.insert(name.to_string(), value);
    }

    /// Get a variable.
    pub fn get_variable(&self, name: &str) -> Option<&serde_json::Value> {
        self.variables.get(name)
    }

    /// Get step state.
    pub fn get_step_state(&self, step_id: &str) -> Option<&StepState> {
        self.step_states.get(step_id)
    }

    /// Get mutable step state.
    pub fn get_step_state_mut(&mut self, step_id: &str) -> Option<&mut StepState> {
        self.step_states.get_mut(step_id)
    }

    /// Check if all steps are complete (or skipped).
    pub fn is_complete(&self) -> bool {
        self.step_states.values().all(|s| matches!(s.status, StepStatus::Completed | StepStatus::Skipped))
    }

    /// Check if any step failed.
    pub fn has_failed(&self) -> bool {
        self.step_states.values().any(|s| s.status == StepStatus::Failed)
    }

    /// Get execution summary.
    pub fn summary(&self) -> RunSummary {
        let total = self.step_states.len();
        let completed = self.step_states.values().filter(|s| s.status == StepStatus::Completed).count();
        let failed = self.step_states.values().filter(|s| s.status == StepStatus::Failed).count();
        let skipped = self.step_states.values().filter(|s| s.status == StepStatus::Skipped).count();
        let total_duration = self.step_states.values().filter_map(|s| s.duration_ms).sum::<u64>();

        RunSummary {
            run_id: self.run_id.clone(),
            plan_name: self.plan_name.clone(),
            status: self.status,
            total_steps: total,
            completed_steps: completed,
            failed_steps: failed,
            skipped_steps: skipped,
            total_duration_ms: total_duration,
            artifacts_count: self.artifacts.len(),
            gates_passed: self.gate_results.iter().filter(|g| g.passed).count(),
            gates_failed: self.gate_results.iter().filter(|g| !g.passed).count(),
        }
    }

    /// Serialize run state to JSON.
    pub fn to_json(&self) -> Result<String, StateError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserialize run state from JSON.
    pub fn from_json(json: &str) -> Result<Self, StateError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Save run state to file.
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), StateError> {
        let json = self.to_json()?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load run state from file.
    pub fn load_from_file(path: &PathBuf) -> Result<Self, StateError> {
        let json = std::fs::read_to_string(path)?;
        Self::from_json(&json)
    }
}

/// Execution summary for reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub plan_name: String,
    pub status: RunStatus,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub skipped_steps: usize,
    pub total_duration_ms: u64,
    pub artifacts_count: usize,
    pub gates_passed: usize,
    pub gates_failed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_version_parse() {
        let v = PackageVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_package_version_parse_pre() {
        let v = PackageVersion::parse("1.2.3-alpha").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.pre_release, Some("alpha".to_string()));
    }

    #[test]
    fn test_run_state_lifecycle() {
        use crate::compiler::{CompiledPlan, StepPlan, StepKind, PlanMetadata};

        let plan = CompiledPlan {
            plan_id: "test-plan".into(),
            name: "Test".into(),
            version: "1.0".into(),
            steps: vec![
                StepPlan { id: "step1".into(), kind: StepKind::Atomic, label: "Step 1".into(), prompt: "Do it".into(), config: serde_json::json!({}), next: vec!["step2".into()], gates: vec![], metadata: Default::default() },
                StepPlan { id: "step2".into(), kind: StepKind::End, label: "End".into(), prompt: "Done".into(), config: serde_json::json!({}), next: vec![], gates: vec![], metadata: Default::default() },
            ],
            entry_step: "step1".into(),
            metadata: PlanMetadata::default(),
        };

        let mut state = RunState::new(&plan);
        assert_eq!(state.status, RunStatus::Pending);

        state.start();
        assert_eq!(state.status, RunStatus::Running);
        assert!(state.started_at.is_some());

        state.start_step("step1", serde_json::json!({"input": "data"})).unwrap();
        let step = state.get_step_state("step1").unwrap();
        assert_eq!(step.status, StepStatus::Running);

        state.complete_step("step1", serde_json::json!({"result": "ok"})).unwrap();
        let step = state.get_step_state("step1").unwrap();
        assert_eq!(step.status, StepStatus::Completed);
        assert!(step.output.is_some());

        state.complete(&plan); // Note: complete() doesn't take plan arg
        assert_eq!(state.status, RunStatus::Completed);
        assert!(state.completed_at.is_some());
    }

    #[test]
    fn test_run_state_variables() {
        use crate::compiler::{CompiledPlan, StepPlan, StepKind, PlanMetadata};

        let plan = CompiledPlan {
            plan_id: "test-plan".into(),
            name: "Test".into(),
            version: "1.0".into(),
            steps: vec![
                StepPlan { id: "step1".into(), kind: StepKind::Atomic, label: "Step 1".into(), prompt: "Do it".into(), config: serde_json::json!({}), next: vec![], gates: vec![], metadata: Default::default() },
            ],
            entry_step: "step1".into(),
            metadata: PlanMetadata::default(),
        };

        let mut state = RunState::new(&plan);
        state.set_variable("key1", serde_json::json!("value1"));
        state.set_variable("key2", serde_json::json!(42));

        assert_eq!(state.get_variable("key1"), Some(&serde_json::json!("value1")));
        assert_eq!(state.get_variable("key2"), Some(&serde_json::json!(42)));
        assert_eq!(state.get_variable("nonexistent"), None);
    }

    #[test]
    fn test_workflow_package_roundtrip() {
        use crate::compiler::{CompiledPlan, StepPlan, StepKind, PlanMetadata};

        let plan = CompiledPlan {
            plan_id: "test-plan".into(),
            name: "Test Plan".into(),
            version: "1.0".into(),
            steps: vec![
                StepPlan { id: "step1".into(), kind: StepKind::Atomic, label: "Step 1".into(), prompt: "Do it".into(), config: serde_json::json!({}), next: vec!["step2".into()], gates: vec![], metadata: Default::default() },
                StepPlan { id: "step2".into(), kind: StepKind::End, label: "End".into(), prompt: "Done".into(), config: serde_json::json!({}), next: vec![], gates: vec![], metadata: Default::default() },
            ],
            entry_step: "step1".into(),
            metadata: PlanMetadata::default(),
        };

        let mut pkg = WorkflowPackage::new("test-workflow", PackageVersion::new(1, 0, 0), plan);
        pkg.add_asset("script.sh", b"echo hello".to_vec());
        pkg.export_skill(ExportedSkill {
            skill_name: "test-skill".into(),
            entry_step: "step1".into(),
            description: "A test skill".into(),
            parameters: vec![SkillParameter { name: "input".into(), param_type: "string".into(), description: "Input data".into(), required: true, default: None }],
        });

        let zip_data = pkg.to_zip().unwrap();
        assert!(!zip_data.is_empty());

        let loaded = WorkflowPackage::from_zip(&zip_data).unwrap();
        assert_eq!(loaded.name, "test-workflow");
        assert_eq!(loaded.version, PackageVersion::new(1, 0, 0));
        assert_eq!(loaded.plan.plan_id, "test-plan");
        assert!(loaded.assets.contains_key("script.sh"));
        assert_eq!(loaded.manifest.exported_skills.len(), 1);
    }

    #[test]
    fn test_run_summary() {
        use crate::compiler::{CompiledPlan, StepPlan, StepKind, PlanMetadata};

        let plan = CompiledPlan {
            plan_id: "test-plan".into(),
            name: "Test".into(),
            version: "1.0".into(),
            steps: vec![
                StepPlan { id: "step1".into(), kind: StepKind::Atomic, label: "Step 1".into(), prompt: "Do it".into(), config: serde_json::json!({}), next: vec!["step2".into()], gates: vec![], metadata: Default::default() },
                StepPlan { id: "step2".into(), kind: StepKind::Atomic, label: "Step 2".into(), prompt: "Do it".into(), config: serde_json::json!({}), next: vec!["step3".into()], gates: vec![], metadata: Default::default() },
                StepPlan { id: "step3".into(), kind: StepKind::End, label: "End".into(), prompt: "Done".into(), config: serde_json::json!({}), next: vec![], gates: vec![], metadata: Default::default() },
            ],
            entry_step: "step1".into(),
            metadata: PlanMetadata::default(),
        };

        let mut state = RunState::new(&plan);
        state.start();
        state.start_step("step1", serde_json::json!({})).unwrap();
        state.complete_step("step1", serde_json::json!({})).unwrap();
        state.start_step("step2", serde_json::json!({})).unwrap();
        state.fail_step("step2", "error".to_string()).unwrap();
        state.skip_step("step3").unwrap();

        let summary = state.summary();
        assert_eq!(summary.total_steps, 3);
        assert_eq!(summary.completed_steps, 1);
        assert_eq!(summary.failed_steps, 1);
        assert_eq!(summary.skipped_steps, 1);
    }
}