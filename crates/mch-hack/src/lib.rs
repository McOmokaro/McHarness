//! mch-hack — autonomous hackathon execution.
//!
//! Orchestrates the full lifecycle: discover hackathons → triage projects →
//! generate code + pitch decks → test → submit.

pub mod crawler;
pub mod generator;
pub mod submitter;
pub mod triage;

use crawler::{HackathonCrawler, PlatformAdapter};
use generator::{CodeGenerator, PitchDeckGenerator};
use submitter::{SubmissionPipeline, PlatformSubmitter};
use triage::{ProjectTriage, RubricEvaluator};

/// High-level facade for the autonomous hackathon agent.
pub struct HackAgent {
    crawler: HackathonCrawler,
    triage: ProjectTriage,
    code_gen: CodeGenerator,
    pitch_gen: PitchDeckGenerator,
    pipeline: SubmissionPipeline,
}

impl HackAgent {
    pub fn new() -> Self {
        Self {
            crawler: HackathonCrawler::new(),
            triage: ProjectTriage::new(),
            code_gen: CodeGenerator::new(),
            pitch_gen: PitchDeckGenerator::new(),
            pipeline: SubmissionPipeline::new(),
        }
    }

    /// Discover hackathons across all configured platforms.
    pub async fn discover(&self) -> anyhow::Result<Vec<crawler::Hackathon>> {
        self.crawler.discover().await
    }

    /// Evaluate a project against competition rubrics.
    pub fn evaluate(&self, project: &triage::Project) -> triage::Evaluation {
        self.triage.evaluate(project)
    }

    /// Generate implementation code for a project.
    pub async fn generate_code(&self, spec: &generator::CodeSpec) -> anyhow::Result<generator::CodeBundle> {
        self.code_gen.generate(spec).await
    }

    /// Generate a 3-minute pitch deck.
    pub async fn generate_pitch(&self, project: &triage::Project) -> anyhow::Result<generator::PitchDeck> {
        self.pitch_gen.generate(project).await
    }

    /// Submit to a platform.
    pub async fn submit(&self, submission: submitter::Submission) -> anyhow::Result<submitter::SubmissionResult> {
        self.pipeline.submit(submission).await
    }
}

impl Default for HackAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn agent_constructs() {
        let _agent = HackAgent::new();
    }
}