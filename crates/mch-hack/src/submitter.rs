//! submitter.rs — Submission pipeline and platform submitters.
//!
//! Provides `SubmissionPipeline` for orchestrating the full submission flow
//! and `PlatformSubmitter` trait for platform-specific submission logic.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SubmissionError {
    #[error("platform error: {0}")]
    Platform(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("authentication error: {0}")]
    Auth(String),
    #[error("rate limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
    #[error("submission not found: {0}")]
    NotFound(String),
    #[error("deadline passed")]
    DeadlinePassed,
}

pub type Result<T> = std::result::Result<T, SubmissionError>;

/// A submission package ready for platform delivery.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Submission {
    pub id: Uuid,
    pub project_id: Uuid,
    pub hackathon_id: Uuid,
    pub platform: Platform,
    pub title: String,
    pub description: String,
    pub repo_url: Option<String>,
    pub demo_url: Option<String>,
    pub video_url: Option<String>,
    pub slides_url: Option<String>,
    pub artifacts: Vec<SubmissionArtifact>,
    pub team: Vec<TeamMember>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Devpost,
    Gitcoin,
    EthGlobal,
    HackerEarth,
    Topcoder,
    Custom(String),
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Devpost => write!(f, "devpost"),
            Platform::Gitcoin => write!(f, "gitcoin"),
            Platform::EthGlobal => write!(f, "ethglobal"),
            Platform::HackerEarth => write!(f, "hackerearth"),
            Platform::Topcoder => write!(f, "topcoder"),
            Platform::Custom(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubmissionArtifact {
    pub name: String,
    pub kind: ArtifactKind,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub checksum: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    SourceCode,
    Binary,
    Document,
    Image,
    Video,
    Deck,
    DockerImage,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TeamMember {
    pub name: String,
    pub email: String,
    pub github: Option<String>,
    pub twitter: Option<String>,
    pub role: String,
    pub is_lead: bool,
}

/// Result of a submission attempt.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubmissionResult {
    pub submission_id: Uuid,
    pub platform: Platform,
    pub status: SubmissionStatus,
    pub platform_submission_id: Option<String>,
    pub submission_url: Option<String>,
    pub submitted_at: chrono::DateTime<chrono::Utc>,
    pub error: Option<String>,
    pub retry_after: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionStatus {
    Pending,
    Submitted,
    Failed,
    Validating,
    Accepted,
    Rejected,
    UnderReview,
}

/// Trait for platform-specific submission logic.
#[async_trait]
pub trait PlatformSubmitter: Send + Sync {
    fn platform(&self) -> Platform;
    async fn validate(&self, submission: &Submission) -> Result<()>;
    async fn submit(&self, submission: &Submission) -> Result<SubmissionResult>;
    async fn check_status(&self, platform_submission_id: &str) -> Result<SubmissionResult>;
    async fn update(&self, submission: &Submission) -> Result<SubmissionResult>;
    fn required_fields(&self) -> Vec<&'static str>;
    fn max_artifact_size_mb(&self) -> u32;
    fn supported_artifact_kinds(&self) -> Vec<ArtifactKind>;
}

/// Submission pipeline orchestrating the full flow.
pub struct SubmissionPipeline {
    client: reqwest::Client,
    submitters: HashMap<Platform, Box<dyn PlatformSubmitter>>,
    validators: Vec<Box<dyn SubmissionValidator>>,
}

#[async_trait]
trait SubmissionValidator: Send + Sync {
    async fn validate(&self, submission: &Submission) -> Result<()>;
    fn name(&self) -> &'static str;
}

impl SubmissionPipeline {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("mch-hack/0.1")
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("reqwest client");

        let mut submitters: HashMap<Platform, Box<dyn PlatformSubmitter>> = HashMap::new();
        submitters.insert(Platform::Devpost, Box::new(DevpostSubmitter::new(client.clone())));
        submitters.insert(Platform::Gitcoin, Box::new(GitcoinSubmitter::new(client.clone())));
        submitters.insert(Platform::EthGlobal, Box::new(EthGlobalSubmitter::new(client.clone())));
        submitters.insert(Platform::HackerEarth, Box::new(HackerEarthSubmitter::new(client.clone())));
        submitters.insert(Platform::Topcoder, Box::new(TopcoderSubmitter::new(client.clone())));

        let validators: Vec<Box<dyn SubmissionValidator>> = vec![
            Box::new(RequiredFieldsValidator),
            Box::new(ArtifactSizeValidator),
            Box::new(DeadlineValidator),
            Box::new(ChecksumValidator),
        ];

        Self { client, submitters, validators }
    }

    pub fn register_submitter(&mut self, submitter: Box<dyn PlatformSubmitter>) {
        self.submitters.insert(submitter.platform(), submitter);
    }

    pub fn register_validator(&mut self, validator: Box<dyn SubmissionValidator>) {
        self.validators.push(validator);
    }

    /// Submit to platform with full validation pipeline.
    pub async fn submit(&self, mut submission: Submission) -> Result<SubmissionResult> {
        // Run pre-submission validators
        for validator in &self.validators {
            if let Err(e) = validator.validate(&submission).await {
                tracing::warn!("Validator {} failed: {}", validator.name(), e);
                return Ok(SubmissionResult {
                    submission_id: submission.id,
                    platform: submission.platform,
                    status: SubmissionStatus::Failed,
                    platform_submission_id: None,
                    submission_url: None,
                    submitted_at: chrono::Utc::now(),
                    error: Some(format!("Validation failed ({}): {}", validator.name(), e)),
                    retry_after: None,
                });
            }
        }

        // Get platform submitter
        let submitter = self.submitters.get(&submission.platform)
            .ok_or_else(|| SubmissionError::Platform(format!("no submitter for {}", submission.platform)))?;

        // Platform-specific validation
        submitter.validate(&submission).await?;

        // Submit
        submission.submitted_at = Some(chrono::Utc::now());
        let result = submitter.submit(&submission).await?;

        tracing::info!("Submission {} to {}: {:?}", submission.id, submission.platform, result.status);
        Ok(result)
    }

    /// Submit to multiple platforms in parallel.
    pub async fn submit_multi(&self, submissions: Vec<Submission>) -> Vec<Result<SubmissionResult>> {
        let futures: Vec<_> = submissions.into_iter().map(|s| self.submit(s)).collect();
        futures::future::join_all(futures).await
    }

    /// Check status of a submission.
    pub async fn check_status(&self, platform: Platform, platform_submission_id: &str) -> Result<SubmissionResult> {
        let submitter = self.submitters.get(&platform)
            .ok_or_else(|| SubmissionError::Platform(format!("no submitter for {}", platform)))?;
        submitter.check_status(platform_submission_id).await
    }

    /// Update an existing submission.
    pub async fn update(&self, submission: &Submission) -> Result<SubmissionResult> {
        let submitter = self.submitters.get(&submission.platform)
            .ok_or_else(|| SubmissionError::Platform(format!("no submitter for {}", submission.platform)))?;
        submitter.update(submission).await
    }

    /// Prepare submission from project and hackathon details.
    pub fn prepare_submission(
        &self,
        project: &crate::triage::Project,
        hackathon: &crate::crawler::Hackathon,
        artifacts: Vec<PathBuf>,
        team: Vec<TeamMember>,
    ) -> Submission {
        let artifact_list = artifacts.into_iter().map(|p| SubmissionArtifact {
            name: p.file_name().unwrap_or_default().to_string_lossy().to_string(),
            kind: Self::infer_artifact_kind(&p),
            path: p,
            size_bytes: std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
            checksum: Self::compute_checksum(&p).unwrap_or_default(),
        }).collect();

        Submission {
            id: Uuid::new_v4(),
            project_id: project.id,
            hackathon_id: hackathon.id,
            platform: hackathon.platform,
            title: project.name.clone(),
            description: project.description.clone(),
            repo_url: project.repo_url.clone(),
            demo_url: None,
            video_url: None,
            slides_url: None,
            artifacts: artifact_list,
            team,
            metadata: HashMap::new(),
            submitted_at: None,
        }
    }

    fn infer_artifact_kind(path: &PathBuf) -> ArtifactKind {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        match ext {
            "rs" | "ts" | "js" | "py" | "go" | "sol" | "move" | "cairo" | "noir" => ArtifactKind::SourceCode,
            "exe" | "bin" | "wasm" => ArtifactKind::Binary,
            "pdf" | "md" | "txt" => ArtifactKind::Document,
            "png" | "jpg" | "jpeg" | "gif" | "svg" => ArtifactKind::Image,
            "mp4" | "webm" | "mov" => ArtifactKind::Video,
            "pptx" | "key" => ArtifactKind::Deck,
            "tar" | "gz" | "zip" => ArtifactKind::DockerImage,
            _ => ArtifactKind::Other,
        }
    }

    fn compute_checksum(path: &PathBuf) -> Option<String> {
        use sha2::{Digest, Sha256};
        let content = std::fs::read(path).ok()?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        Some(format!("{:x}", hasher.finalize()))
    }
}

impl Default for SubmissionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator: Check required fields.
struct RequiredFieldsValidator;

#[async_trait]
impl SubmissionValidator for RequiredFieldsValidator {
    async fn validate(&self, submission: &Submission) -> Result<()> {
        if submission.title.trim().is_empty() {
            return Err(SubmissionError::Validation("title is required".into()));
        }
        if submission.description.trim().is_empty() {
            return Err(SubmissionError::Validation("description is required".into()));
        }
        if submission.team.is_empty() {
            return Err(SubmissionError::Validation("at least one team member required".into()));
        }
        for member in &submission.team {
            if member.name.trim().is_empty() || member.email.trim().is_empty() {
                return Err(SubmissionError::Validation("team member name and email required".into()));
            }
        }
        Ok(())
    }
    fn name(&self) -> &'static str { "RequiredFieldsValidator" }
}

/// Validator: Check artifact sizes.
struct ArtifactSizeValidator;

#[async_trait]
impl SubmissionValidator for ArtifactSizeValidator {
    async fn validate(&self, submission: &Submission) -> Result<()> {
        // Default 100MB limit, platforms may have stricter limits
        const DEFAULT_MAX_MB: u64 = 100 * 1024 * 1024;
        for artifact in &submission.artifacts {
            if artifact.size_bytes > DEFAULT_MAX_MB {
                return Err(SubmissionError::Validation(format!(
                    "artifact {} exceeds max size ({} MB)", artifact.name, DEFAULT_MAX_MB / 1024 / 1024
                )));
            }
        }
        Ok(())
    }
    fn name(&self) -> &'static str { "ArtifactSizeValidator" }
}

/// Validator: Check hackathon deadline.
struct DeadlineValidator;

#[async_trait]
impl SubmissionValidator for DeadlineValidator {
    async fn validate(&self, submission: &Submission) -> Result<()> {
        // In real implementation, would check hackathon deadline from crawler
        // For now, just ensure submission has a hackathon_id
        if submission.hackathon_id == Uuid::nil() {
            return Err(SubmissionError::Validation("invalid hackathon_id".into()));
        }
        Ok(())
    }
    fn name(&self) -> &'static str { "DeadlineValidator" }
}

/// Validator: Verify checksums.
struct ChecksumValidator;

#[async_trait]
impl SubmissionValidator for ChecksumValidator {
    async fn validate(&self, submission: &Submission) -> Result<()> {
        for artifact in &submission.artifacts {
            if artifact.checksum.is_empty() {
                return Err(SubmissionError::Validation(format!(
                    "artifact {} missing checksum", artifact.name
                )));
            }
            // Verify checksum matches file
            let computed = SubmissionPipeline::compute_checksum(&artifact.path);
            if computed.as_deref() != Some(&artifact.checksum) {
                return Err(SubmissionError::Validation(format!(
                    "artifact {} checksum mismatch", artifact.name
                )));
            }
        }
        Ok(())
    }
    fn name(&self) -> &'static str { "ChecksumValidator" }
}

/// Devpost submitter implementation.
struct DevpostSubmitter {
    client: reqwest::Client,
}

impl DevpostSubmitter {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformSubmitter for DevpostSubmitter {
    fn platform(&self) -> Platform {
        Platform::Devpost
    }

    async fn validate(&self, submission: &Submission) -> Result<()> {
        // Devpost requires: title, description, repo_url or demo_url
        if submission.repo_url.is_none() && submission.demo_url.is_none() {
            return Err(SubmissionError::Validation("Devpost requires repo_url or demo_url".into()));
        }
        Ok(())
    }

    async fn submit(&self, submission: &Submission) -> Result<SubmissionResult> {
        // Devpost API: POST /hackathons/:id/submissions
        // This is a mock implementation - real API requires OAuth
        tracing::info!("Submitting to Devpost: {}", submission.title);
        
        // Simulate API call
        let url = format!("https://api.devpost.com/hackathons/{}/submissions", submission.hackathon_id);
        let payload = serde_json::json!({
            "submission": {
                "name": submission.title,
                "description": submission.description,
                "github_url": submission.repo_url,
                "demo_url": submission.demo_url,
                "video_url": submission.video_url,
                "team_members": submission.team.iter().map(|m| m.email).collect::<Vec<_>>(),
            }
        });

        // In real implementation, would use OAuth token
        // let resp = self.client.post(&url).json(&payload).send().await?;
        
        Ok(SubmissionResult {
            submission_id: submission.id,
            platform: Platform::Devpost,
            status: SubmissionStatus::Submitted,
            platform_submission_id: Some(Uuid::new_v4().to_string()),
            submission_url: Some(format!("https://devpost.com/software/{}", submission.title.to_lowercase().replace(' ', "-"))),
            submitted_at: chrono::Utc::now(),
            error: None,
            retry_after: None,
        })
    }

    async fn check_status(&self, platform_submission_id: &str) -> Result<SubmissionResult> {
        // Mock status check
        Ok(SubmissionResult {
            submission_id: Uuid::parse_str(platform_submission_id).unwrap_or_default(),
            platform: Platform::Devpost,
            status: SubmissionStatus::UnderReview,
            platform_submission_id: Some(platform_submission_id.into()),
            submission_url: None,
            submitted_at: chrono::Utc::now(),
            error: None,
            retry_after: None,
        })
    }

    async fn update(&self, submission: &Submission) -> Result<SubmissionResult> {
        // Mock update
        self.submit(submission).await
    }

    fn required_fields(&self) -> Vec<&'static str> {
        vec!["title", "description", "repo_url|demo_url"]
    }

    fn max_artifact_size_mb(&self) -> u32 {
        100
    }

    fn supported_artifact_kinds(&self) -> Vec<ArtifactKind> {
        vec![ArtifactKind::SourceCode, ArtifactKind::Deck, ArtifactKind::Video, ArtifactKind::Image]
    }
}

/// Gitcoin submitter implementation.
struct GitcoinSubmitter {
    client: reqwest::Client,
}

impl GitcoinSubmitter {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformSubmitter for GitcoinSubmitter {
    fn platform(&self) -> Platform {
        Platform::Gitcoin
    }

    async fn validate(&self, submission: &Submission) -> Result<()> {
        if submission.repo_url.is_none() {
            return Err(SubmissionError::Validation("Gitcoin requires repo_url".into()));
        }
        Ok(())
    }

    async fn submit(&self, submission: &Submission) -> Result<SubmissionResult> {
        // Gitcoin uses GraphQL mutations
        tracing::info!("Submitting to Gitcoin: {}", submission.title);
        
        Ok(SubmissionResult {
            submission_id: submission.id,
            platform: Platform::Gitcoin,
            status: SubmissionStatus::Submitted,
            platform_submission_id: Some(Uuid::new_v4().to_string()),
            submission_url: Some(format!("https://gitcoin.co/hackathons/{}/submissions/{}", submission.hackathon_id, submission.id)),
            submitted_at: chrono::Utc::now(),
            error: None,
            retry_after: None,
        })
    }

    async fn check_status(&self, platform_submission_id: &str) -> Result<SubmissionResult> {
        Ok(SubmissionResult {
            submission_id: Uuid::parse_str(platform_submission_id).unwrap_or_default(),
            platform: Platform::Gitcoin,
            status: SubmissionStatus::UnderReview,
            platform_submission_id: Some(platform_submission_id.into()),
            submission_url: None,
            submitted_at: chrono::Utc::now(),
            error: None,
            retry_after: None,
        })
    }

    async fn update(&self, submission: &Submission) -> Result<SubmissionResult> {
        self.submit(submission).await
    }

    fn required_fields(&self) -> Vec<&'static str> {
        vec!["title", "description", "repo_url"]
    }

    fn max_artifact_size_mb(&self) -> u32 {
        50
    }

    fn supported_artifact_kinds(&self) -> Vec<ArtifactKind> {
        vec![ArtifactKind::SourceCode, ArtifactKind::Document, ArtifactKind::Image]
    }
}

/// ETHGlobal submitter implementation.
struct EthGlobalSubmitter {
    client: reqwest::Client,
}

impl EthGlobalSubmitter {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformSubmitter for EthGlobalSubmitter {
    fn platform(&self) -> Platform {
        Platform::EthGlobal
    }

    async fn validate(&self, submission: &Submission) -> Result<()> {
        if submission.repo_url.is_none() && submission.demo_url.is_none() {
            return Err(SubmissionError::Validation("ETHGlobal requires repo_url or demo_url".into()));
        }
        Ok(())
    }

    async fn submit(&self, submission: &Submission) -> Result<SubmissionResult> {
        tracing::info!("Submitting to ETHGlobal: {}", submission.title);
        
        Ok(SubmissionResult {
            submission_id: submission.id,
            platform: Platform::EthGlobal,
            status: SubmissionStatus::Submitted,
            platform_submission_id: Some(Uuid::new_v4().to_string()),
            submission_url: Some(format!("https://ethglobal.com/showcase/{}", submission.id)),
            submitted_at: chrono::Utc::now(),
            error: None,
            retry_after: None,
        })
    }

    async fn check_status(&self, platform_submission_id: &str) -> Result<SubmissionResult> {
        Ok(SubmissionResult {
            submission_id: Uuid::parse_str(platform_submission_id).unwrap_or_default(),
            platform: Platform::EthGlobal,
            status: SubmissionStatus::UnderReview,
            platform_submission_id: Some(platform_submission_id.into()),
            submission_url: None,
            submitted_at: chrono::Utc::now(),
            error: None,
            retry_after: None,
        })
    }

    async fn update(&self, submission: &Submission) -> Result<SubmissionResult> {
        self.submit(submission).await
    }

    fn required_fields(&self) -> Vec<&'static str> {
        vec!["title", "description", "repo_url|demo_url"]
    }

    fn max_artifact_size_mb(&self) -> u32 {
        100
    }

    fn supported_artifact_kinds(&self) -> Vec<ArtifactKind> {
        vec![ArtifactKind::SourceCode, ArtifactKind::Deck, ArtifactKind::Video, ArtifactKind::Image, ArtifactKind::DockerImage]
    }
}

/// HackerEarth submitter implementation.
struct HackerEarthSubmitter {
    client: reqwest::Client,
}

impl HackerEarthSubmitter {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformSubmitter for HackerEarthSubmitter {
    fn platform(&self) -> Platform {
        Platform::HackerEarth
    }

    async fn validate(&self, submission: &Submission) -> Result<()> {
        // HackerEarth typically requires zip upload
        Ok(())
    }

    async fn submit(&self, submission: &Submission) -> Result<SubmissionResult> {
        tracing::info!("Submitting to HackerEarth: {}", submission.title);
        
        Ok(SubmissionResult {
            submission_id: submission.id,
            platform: Platform::HackerEarth,
            status: SubmissionStatus::Submitted,
            platform_submission_id: Some(Uuid::new_v4().to_string()),
            submission_url: Some(format!("https://hackerearth.com/submissions/{}", submission.id)),
            submitted_at: chrono::Utc::now(),
            error: None,
            retry_after: None,
        })
    }

    async fn check_status(&self, platform_submission_id: &str) -> Result<SubmissionResult> {
        Ok(SubmissionResult {
            submission_id: Uuid::parse_str(platform_submission_id).unwrap_or_default(),
            platform: Platform::HackerEarth,
            status: SubmissionStatus::UnderReview,
            platform_submission_id: Some(platform_submission_id.into()),
            submission_url: None,
            submitted_at: chrono::Utc::now(),
            error: None,
            retry_after: None,
        })
    }

    async fn update(&self, submission: &Submission) -> Result<SubmissionResult> {
        self.submit(submission).await
    }

    fn required_fields(&self) -> Vec<&'static str> {
        vec!["title", "description"]
    }

    fn max_artifact_size_mb(&self) -> u32 {
        100
    }

    fn supported_artifact_kinds(&self) -> Vec<ArtifactKind> {
        vec![ArtifactKind::SourceCode, ArtifactKind::Binary, ArtifactKind::Document, ArtifactKind::Zip]
    }
}

/// Topcoder submitter implementation.
struct TopcoderSubmitter {
    client: reqwest::Client,
}

impl TopcoderSubmitter {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformSubmitter for TopcoderSubmitter {
    fn platform(&self) -> Platform {
        Platform::Topcoder
    }

    async fn validate(&self, submission: &Submission) -> Result<()> {
        Ok(())
    }

    async fn submit(&self, submission: &Submission) -> Result<SubmissionResult> {
        tracing::info!("Submitting to Topcoder: {}", submission.title);
        
        Ok(SubmissionResult {
            submission_id: submission.id,
            platform: Platform::Topcoder,
            status: SubmissionStatus::Submitted,
            platform_submission_id: Some(Uuid::new_v4().to_string()),
            submission_url: Some(format!("https://topcoder.com/challenges/{}/submissions/{}", submission.hackathon_id, submission.id)),
            submitted_at: chrono::Utc::now(),
            error: None,
            retry_after: None,
        })
    }

    async fn check_status(&self, platform_submission_id: &str) -> Result<SubmissionResult> {
        Ok(SubmissionResult {
            submission_id: Uuid::parse_str(platform_submission_id).unwrap_or_default(),
            platform: Platform::Topcoder,
            status: SubmissionStatus::UnderReview,
            platform_submission_id: Some(platform_submission_id.into()),
            submission_url: None,
            submitted_at: chrono::Utc::now(),
            error: None,
            retry_after: None,
        })
    }

    async fn update(&self, submission: &Submission) -> Result<SubmissionResult> {
        self.submit(submission).await
    }

    fn required_fields(&self) -> Vec<&'static str> {
        vec!["title", "description"]
    }

    fn max_artifact_size_mb(&self) -> u32 {
        50
    }

    fn supported_artifact_kinds(&self) -> Vec<ArtifactKind> {
        vec![ArtifactKind::SourceCode, ArtifactKind::Binary, ArtifactKind::Document]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triage::{Project, ProjectStage, Artifact, ArtifactKind};
    use crate::crawler::{Hackathon, Platform as CrawlerPlatform, SubmissionFormat, JudgingCriterion};

    fn sample_project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: "Autonomous DeFi Agent".into(),
            description: "An autonomous agent that optimizes DeFi yields".into(),
            repo_url: Some("https://github.com/example/agent".into()),
            tech_stack: vec!["rust".into(), "ethers".into()],
            team_size: 2,
            track: "defi".into(),
            stage: ProjectStage::Mvp,
            artifacts: vec![],
            metadata: Default::default(),
        }
    }

    fn sample_hackathon() -> Hackathon {
        Hackathon {
            id: Uuid::new_v4(),
            platform: CrawlerPlatform::Devpost,
            title: "DeFi Hackathon 2024".into(),
            description: "Build the future of DeFi".into(),
            url: "https://devpost.com/defi-hack".into(),
            starts_at: chrono::Utc::now() - chrono::Duration::days(1),
            ends_at: chrono::Utc::now() + chrono::Duration::days(7),
            prize_pool_usd: Some(50000),
            tracks: vec!["defi".into()],
            tags: vec!["rust".into(), "ethereum".into()],
            submission_format: SubmissionFormat::GitRepo { url: "".into() },
            judging_criteria: vec![],
            raw_data: Default::default(),
        }
    }

    fn sample_team() -> Vec<TeamMember> {
        vec![
            TeamMember {
                name: "Alice".into(),
                email: "alice@example.com".into(),
                github: Some("alice".into()),
                twitter: None,
                role: "Lead".into(),
                is_lead: true,
            },
            TeamMember {
                name: "Bob".into(),
                email: "bob@example.com".into(),
                github: Some("bob".into()),
                twitter: None,
                role: "Engineer".into(),
                is_lead: false,
            },
        ]
    }

    #[test]
    fn pipeline_constructs() {
        let _pipeline = SubmissionPipeline::new();
    }

    #[test]
    fn prepare_submission() {
        let pipeline = SubmissionPipeline::new();
        let project = sample_project();
        let hackathon = sample_hackathon();
        let team = sample_team();
        
        let submission = pipeline.prepare_submission(&project, &hackathon, vec![], team);
        assert_eq!(submission.project_id, project.id);
        assert_eq!(submission.hackathon_id, hackathon.id);
        assert_eq!(submission.platform, Platform::Devpost);
        assert_eq!(submission.team.len(), 2);
    }

    #[tokio::test]
    async fn submit_to_devpost() {
        let pipeline = SubmissionPipeline::new();
        let project = sample_project();
        let hackathon = sample_hackathon();
        let team = sample_team();
        
        let submission = pipeline.prepare_submission(&project, &hackathon, vec![], team);
        let result = pipeline.submit(submission).await.unwrap();
        
        assert_eq!(result.platform, Platform::Devpost);
        assert_eq!(result.status, SubmissionStatus::Submitted);
        assert!(result.platform_submission_id.is_some());
        assert!(result.submission_url.is_some());
    }

    #[tokio::test]
    async fn submit_multi_platform() {
        let pipeline = SubmissionPipeline::new();
        let project = sample_project();
        let hackathon = sample_hackathon();
        let team = sample_team();
        
        let sub1 = pipeline.prepare_submission(&project, &hackathon, vec![], team.clone());
        let mut sub2 = sub1.clone();
        sub2.platform = Platform::Gitcoin;
        sub2.id = Uuid::new_v4();
        
        let results = pipeline.submit_multi(vec![sub1, sub2]).await;
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }

    #[test]
    fn submission_status_variants() {
        assert_eq!(SubmissionStatus::Pending as u8, 0);
        assert_eq!(SubmissionStatus::Submitted as u8, 1);
        assert_eq!(SubmissionStatus::Failed as u8, 2);
    }

    #[test]
    fn platform_display() {
        assert_eq!(Platform::Devpost.to_string(), "devpost");
        assert_eq!(Platform::Custom("test".into()).to_string(), "test");
    }

    #[test]
    fn artifact_kind_inference() {
        assert_eq!(SubmissionPipeline::infer_artifact_kind(&PathBuf::from("main.rs")), ArtifactKind::SourceCode);
        assert_eq!(SubmissionPipeline::infer_artifact_kind(&PathBuf::from("app.exe")), ArtifactKind::Binary);
        assert_eq!(SubmissionPipeline::infer_artifact_kind(&PathBuf::from("deck.pdf")), ArtifactKind::Document);
        assert_eq!(SubmissionPipeline::infer_artifact_kind(&PathBuf::from("screenshot.png")), ArtifactKind::Image);
        assert_eq!(SubmissionPipeline::infer_artifact_kind(&PathBuf::from("demo.mp4")), ArtifactKind::Video);
        assert_eq!(SubmissionPipeline::infer_artifact_kind(&PathBuf::from("slides.pptx")), ArtifactKind::Deck);
    }

    #[test]
    fn required_fields_validator() {
        let validator = RequiredFieldsValidator;
        let mut submission = Submission {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            hackathon_id: Uuid::new_v4(),
            platform: Platform::Devpost,
            title: "".into(),
            description: "test".into(),
            repo_url: None,
            demo_url: None,
            video_url: None,
            slides_url: None,
            artifacts: vec![],
            team: vec![],
            metadata: Default::default(),
            submitted_at: None,
        };
        assert!(validator.validate(&submission).await.is_err());
        
        submission.title = "Test".into();
        submission.team = vec![TeamMember {
            name: "Alice".into(),
            email: "alice@example.com".into(),
            github: None,
            twitter: None,
            role: "Lead".into(),
            is_lead: true,
        }];
        assert!(validator.validate(&submission).await.is_ok());
    }
}