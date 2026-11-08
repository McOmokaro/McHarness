//! Procedural Memory — Skill Store with HyDE Retrieval

use crate::types::{RecordId, SessionId};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Skill entry — reusable how-to knowledge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub skill_id: RecordId,
    /// Short description of what the skill does
    pub intent: String,
    /// Full Markdown document describing procedure, commands, parameters, caveats
    pub doc_markdown: String,
    /// Dense vector of intent text (for similarity search)
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
    /// Usage counters
    pub usage_count: u64,
    /// Last used timestamp
    pub last_used: Option<DateTime<Utc>>,
    /// Preconditions for skill applicability
    pub preconditions: Vec<String>,
    /// Source session
    pub session_id: SessionId,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl SkillEntry {
    pub fn new(intent: String, doc_markdown: String, session_id: SessionId) -> Self {
        let skill_id = RecordId::from_content(&intent);
        Self {
            skill_id,
            intent,
            doc_markdown,
            embedding: None,
            usage_count: 0,
            last_used: None,
            preconditions: Vec::new(),
            session_id,
            created_at: Utc::now(),
        }
    }
}

/// Skill store configuration
#[derive(Debug, Clone)]
pub struct SkillConfig {
    pub max_skills: usize,
    pub hyde_model: String,
    pub similarity_threshold: f32,
}

impl Default for SkillConfig {
    fn default() -> Self {
        Self {
            max_skills: 10000,
            hyde_model: "gpt-4o-mini".to_string(),
            similarity_threshold: 0.7,
        }
    }
}

/// Skill store with HyDE retrieval
pub struct SkillStore {
    config: SkillConfig,
    skills: Arc<RwLock<HashMap<RecordId, SkillEntry>>>,
    // In practice, would use LanceDB for vector index
    intent_index: Arc<RwLock<Vec<SkillEntry>>>,
}

impl SkillStore {
    pub fn new(config: SkillConfig) -> Self {
        Self {
            config,
            skills: Arc::new(RwLock::new(HashMap::new())),
            intent_index: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a skill entry
    pub async fn add_skill(&self, skill: SkillEntry) -> Result<()> {
        let mut skills = self.skills.write().await;
        skills.insert(skill.skill_id.clone(), skill.clone());
        drop(skills);
        
        let mut index = self.intent_index.write().await;
        index.push(skill.clone());
        info!("Added skill: {}", skill.intent);
        Ok(())
    }

    /// Get skill by ID
    pub async fn get_skill(&self, id: &RecordId) -> Option<SkillEntry> {
        self.skills.read().await.get(id).cloned()
    }

    /// List all skills
    pub async fn list_skills(&self) -> Vec<SkillEntry> {
        self.skills.read().await.values().cloned().collect()
    }

    /// HyDE retrieval — expand query into hypothetical ideal answer, then embed
    pub async fn hyde_retrieve(&self, query: &str, mode: SkillMode, top_k: usize) -> Vec<SkillEntry> {
        // Step 1: Generate hypothetical answer using LLM
        let hypothetical = self.generate_hypothetical_answer(query, mode).await;
        
        // Step 2: Embed hypothetical answer
        // let query_embedding = embed(&hypothetical).await;
        
        // Step 3: Vector similarity search against skill intents
        // For now, use simple text matching
        let skills = self.intent_index.read().await;
        let mut results: Vec<(SkillEntry, f32)> = skills.iter()
            .filter(|s| mode.matches(&s.intent))
            .map(|s| {
                let score = self.text_similarity(&hypothetical, &s.intent);
                (s.clone(), score)
            })
            .filter(|(_, score)| *score >= self.config.similarity_threshold)
            .collect();
        
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(top_k);
        
        results.into_iter().map(|(s, _)| s).collect()
    }

    async fn generate_hypothetical_answer(&self, query: &str, mode: SkillMode) -> String {
        // In practice, call LLM with HyDE prompt
        // For now, return a template based on mode
        match mode {
            SkillMode::Answer => format!("To answer '{}', I would need to...", query),
            SkillMode::Action => format!("To perform '{}', the steps are: 1. ... 2. ... 3. ...", query),
            SkillMode::Mixed => format!("For '{}', both information and action are needed...", query),
        }
    }

    fn text_similarity(&self, a: &str, b: &str) -> f32 {
        // Simple Jaccard similarity on words
        let words_a: std::collections::HashSet<_> = a.split_whitespace().collect();
        let words_b: std::collections::HashSet<_> = b.split_whitespace().collect();
        let intersection = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();
        if union == 0 { 0.0 } else { intersection as f32 / union as f32 }
    }

    /// Record skill usage for RL signals
    pub async fn record_usage(&self, skill_id: &RecordId, success: bool) {
        let mut skills = self.skills.write().await;
        if let Some(skill) = skills.get_mut(skill_id) {
            skill.usage_count += 1;
            skill.last_used = Some(Utc::now());
            debug!("Skill {} usage: {} (success: {})", skill.intent, skill.usage_count, success);
        }
    }

    /// Extract skills from successful conversation patterns
    pub async fn extract_skills_from_conversation(&self, turns: &[crate::working::Turn]) -> Vec<SkillEntry> {
        // In practice, use LLM to identify reusable patterns
        // For now, return empty
        Vec::new()
    }
}

/// Skill retrieval mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillMode {
    Answer,  // Information-seeking
    Action,  // Action-oriented
    Mixed,   // Both
}

impl SkillMode {
    fn matches(&self, intent: &str) -> bool {
        match self {
            SkillMode::Answer => intent.contains("how to") || intent.contains("what is") || intent.contains("explain"),
            SkillMode::Action => intent.contains("run") || intent.contains("execute") || intent.contains("deploy") || intent.contains("backup"),
            SkillMode::Mixed => true,
        }
    }
}