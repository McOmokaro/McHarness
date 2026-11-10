//! Submission pipeline and platform submitters.

use serde::{Deserialize, Serialize};

/// A submission ready to be sent to a platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    pub hackathon_id: String,
    pub project_name: String,
    pub repo_url: String,
    pub demo_url: Option<String>,
    pub description: String,
    pub team_members: Vec<String>,
    pub artifacts: Vec<Artifact>,
}

/// A submission artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub path: String,
}

/// The kind of an artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Repo,
    Demo,
    Video,
    Deck,
    Zip,
    Other,
}

/// Result of a submission attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionResult {
    pub success: bool,
    pub submission_id: Option<String>,
    pub message: String,
}

/// A platform submitter.
pub trait PlatformSubmitter: Send + Sync {
    fn platform_name(&self) -> &str;
    fn validate(&self, submission: &Submission) -> Result<(), String>;
    fn submit(&self, submission: &Submission) -> SubmissionResult;
}

/// Orchestrates validation + submission to a platform.
pub struct SubmissionPipeline {
    submitters: std::collections::HashMap<String, Box<dyn PlatformSubmitter>>,
}

impl SubmissionPipeline {
    pub fn new() -> Self {
        Self {
            submitters: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, submitter: Box<dyn PlatformSubmitter>) {
        self.submitters
            .insert(submitter.platform_name().to_string(), submitter);
    }

    /// Validate against `platform` and submit (stub: succeeds when registered).
    pub async fn submit(&self, submission: Submission) -> anyhow::Result<SubmissionResult> {
        if submission.project_name.is_empty() || submission.repo_url.is_empty() {
            return Ok(SubmissionResult {
                success: false,
                submission_id: None,
                message: "missing project_name or repo_url".into(),
            });
        }
        // Route to any registered submitter; default to a synthetic success.
        if let Some(submitter) = self.submitters.values().next() {
            if let Err(e) = submitter.validate(&submission) {
                return Ok(SubmissionResult {
                    success: false,
                    submission_id: None,
                    message: e,
                });
            }
            return Ok(submitter.submit(&submission));
        }
        Ok(SubmissionResult {
            success: true,
            submission_id: Some(uuid::Uuid::new_v4().to_string()),
            message: "submitted (no adapter registered; synthetic success)".into(),
        })
    }
}

impl Default for SubmissionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn synthetic_submission_succeeds() {
        let pipeline = SubmissionPipeline::new();
        let sub = Submission {
            hackathon_id: "h1".into(),
            project_name: "Demo".into(),
            repo_url: "https://github.com/x/y".into(),
            demo_url: None,
            description: "d".into(),
            team_members: vec![],
            artifacts: vec![],
        };
        let res = pipeline.submit(sub).await.unwrap();
        assert!(res.success);
    }

    #[tokio::test]
    async fn rejects_incomplete_submission() {
        let pipeline = SubmissionPipeline::new();
        let sub = Submission {
            hackathon_id: "h1".into(),
            project_name: "".into(),
            repo_url: "".into(),
            demo_url: None,
            description: "d".into(),
            team_members: vec![],
            artifacts: vec![],
        };
        assert!(!pipeline.submit(sub).await.unwrap().success);
    }
}
