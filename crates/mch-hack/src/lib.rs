//! mch-hack — autonomous hackathon execution.
//!
//! Orchestrates the full lifecycle: discover hackathons → triage projects →
//! generate code + pitch decks → test → submit.

pub mod crawler;
pub mod generator;
pub mod submitter;
pub mod triage;

pub use crawler::{Hackathon, HackathonCrawler, Platform, PlatformAdapter};
pub use generator::{CodeBundle, CodeGenerator, CodeSpec, PitchDeck, PitchDeckGenerator};
pub use submitter::{
    Artifact, ArtifactKind, PlatformSubmitter, Submission, SubmissionPipeline, SubmissionResult,
};
pub use triage::{Evaluation, Project, ProjectStage, ProjectTriage, RubricEvaluator};

/// High-level facade for the autonomous hackathon agent.
pub struct HackAgent {
    crawler: crawler::HackathonCrawler,
    triage: triage::ProjectTriage,
    code_gen: generator::CodeGenerator,
    pitch_gen: generator::PitchDeckGenerator,
    pipeline: submitter::SubmissionPipeline,
}

impl HackAgent {
    pub fn new() -> Self {
        Self {
            crawler: crawler::HackathonCrawler::new(),
            triage: triage::ProjectTriage::new(),
            code_gen: generator::CodeGenerator::new(),
            pitch_gen: generator::PitchDeckGenerator::new(),
            pipeline: submitter::SubmissionPipeline::new(),
        }
    }

    /// Register a platform adapter for discovery.
    pub fn register_platform(&mut self, adapter: Box<dyn crawler::PlatformAdapter>) {
        self.crawler.register_adapter(adapter);
    }

    /// Register a platform submitter.
    pub fn register_submitter(&mut self, submitter: Box<dyn submitter::PlatformSubmitter>) {
        self.pipeline.register(submitter);
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
    async fn agent_constructs_and_evaluates() {
        let agent = HackAgent::new();
        let ev = agent.evaluate(&Project::new("Demo"));
        assert!(ev.max_total > 0.0);
    }
}
