//! mch-anatomy — Human Archetypes, Cognitive Workflows & Behavioral Heuristics
//!
//! The Anatomy Department models human interaction patterns for LLM agent emulation.
//! Three core components:
//!
//! 1. **Cognitive Personas** (`personas`) — Aggressive trader, polite meeting scheduler,
//!    meticulous bug hunter, and other archetypal interaction styles
//! 2. **Behavioral Heuristics** (`heuristics`) — How to respond to friction, bypass blocks,
//!    handle ambiguity, and recover from failures
//! 3. **Workflow Extraction** (`workflow`) — Ingest screen recordings/transcripts →
//!    map to procedural memory for skill synthesis
//!
//! Integration with `mch-brain` provides procedural memory storage via `SkillStore`.

pub mod heuristics;
pub mod personas;
pub mod workflow;

pub use heuristics::{BehavioralHeuristic, FrictionResponse, FrictionType, HeuristicContext, HeuristicOutcome};
pub use personas::{Archetype, CognitivePersona, PersonaConfig, PersonaRegistry, PersonaTrait};
pub use workflow::{TranscriptParser, WorkflowExtractor, WorkflowRecord, WorkflowStep};

#[cfg(feature = "procedural-memory")]
use anyhow::Result;
#[cfg(feature = "procedural-memory")]
use mch_brain::{SkillEntry, SkillStore, types::SessionId};

/// Anatomy department — coordinates personas, heuristics, and workflow extraction
#[cfg(feature = "procedural-memory")]
pub struct AnatomyDepartment {
    persona_registry: PersonaRegistry,
    skill_store: SkillStore,
}

#[cfg(feature = "procedural-memory")]
impl AnatomyDepartment {
    /// Create a new AnatomyDepartment with default configuration
    pub fn new(skill_store: SkillStore) -> Self {
        Self {
            persona_registry: PersonaRegistry::default(),
            skill_store,
        }
    }

    /// Get a reference to the persona registry
    pub fn personas(&self) -> &PersonaRegistry {
        &self.persona_registry
    }

    /// Get a mutable reference to the persona registry
    pub fn personas_mut(&mut self) -> &mut PersonaRegistry {
        &mut self.persona_registry
    }

    /// Get a reference to the skill store for procedural memory
    pub fn skill_store(&self) -> &SkillStore {
        &self.skill_store
    }

    /// Extract and store a workflow as a procedural skill
    pub async fn learn_workflow(
        &self,
        transcript: &str,
        session_id: SessionId,
    ) -> Result<Vec<SkillEntry>> {
        let extractor = WorkflowExtractor::new();
        let workflow = extractor.extract(transcript)?;

        let mut skills = Vec::new();
        for step in workflow.steps {
            let skill = SkillEntry::new(
                step.intent.clone(),
                format!("{}\n\nSource: {}\nContext: {}", step.procedure, workflow.source_type, step.context),
                session_id,
            );
            self.skill_store.add_skill(skill.clone()).await?;
            skills.push(skill);
        }

        Ok(skills)
    }

    /// Select the best persona for a given context
    pub fn select_persona(&self, context: &HeuristicContext) -> Option<&CognitivePersona> {
        self.persona_registry.select_best(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "procedural-memory")]
    use mch_brain::{SkillConfig, SkillStore};

    #[tokio::test]
    #[cfg(feature = "procedural-memory")]
    async fn test_anatomy_department_creation() {
        let skill_store = SkillStore::new(SkillConfig::default());
        let dept = AnatomyDepartment::new(skill_store);
        assert!(!dept.personas().list().is_empty());
    }

    #[tokio::test]
    async fn test_personas_module_works_without_feature() {
        let registry = PersonaRegistry::new();
        assert!(!registry.list().is_empty());
    }
}