//! Brain Pipeline — 5-Stage Agent Pipeline (from LycheeMem)

use crate::curator::{CuratorDaemon, CuratorConfig};
use crate::memory::MemoryTree;
use crate::retrieval::{RetrievalEngine, RetrievalConfig};
use crate::skills::{SkillStore, SkillEntry, SkillMode};
use crate::storage::{BrainStorage, UnifiedStore, StorageConfig};
use crate::types::{SessionId, ActionOutcome, ConsolidationTrigger, SearchPlan, ActionState, RetrievalMode, MemoryRecord};
use crate::visual::{VisualMemory, VisualConfig};
use crate::working::{WorkingMemory, WorkingConfig, CompressedHistory};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

/// Pipeline configuration
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub working: WorkingConfig,
    pub retrieval: RetrievalConfig,
    pub storage: StorageConfig,
    pub visual: VisualConfig,
    pub curator: CuratorConfig,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            working: WorkingConfig::default(),
            retrieval: RetrievalConfig::default(),
            storage: StorageConfig::default(),
            visual: VisualConfig::default(),
            curator: CuratorConfig::default(),
        }
    }
}

/// Pipeline stage result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub stage: PipelineStage,
    pub success: bool,
    pub duration_ms: u64,
    pub metadata: serde_json::Value,
}

/// Pipeline stages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStage {
    WmManager,
    SearchCoordinator,
    SynthesizerAgent,
    ReasoningAgent,
    ConsolidatorAgent,
}

/// Brain pipeline orchestrating all 5 stages
pub struct BrainPipeline {
    config: PipelineConfig,
    working: Arc<RwLock<WorkingMemory>>,
    retrieval: Arc<RetrievalEngine>,
    storage: Arc<UnifiedStore>,
    skills: Arc<SkillStore>,
    visual: Arc<VisualMemory>,
    curator: Arc<CuratorDaemon>,
    session_id: SessionId,
}

impl BrainPipeline {
    /// Create a new brain pipeline
    pub async fn new(config: PipelineConfig) -> Result<Self> {
        let session_id = SessionId::new();
        
        let working = Arc::new(RwLock::new(WorkingMemory::new(config.working.clone())?));
        let storage = Arc::new(UnifiedStore::new(config.storage.clone()).await?);
        let retrieval = Arc::new(RetrievalEngine::new(config.retrieval.clone()));
        let skills = Arc::new(SkillStore::new(crate::skills::SkillConfig::default()));
        let visual = Arc::new(VisualMemory::new(config.visual.clone()));
        
        let curator = Arc::new(CuratorDaemon::new(
            config.curator.clone(),
            storage.clone(),
            retrieval.clone(),
            skills.clone(),
            visual.clone(),
            working.clone(),
        ));

        // Build initial retrieval indices
        let tree = storage.get_tree().await?;
        retrieval.build_indices(&tree);

        // Start curator daemon
        let curator_clone = curator.clone();
        tokio::spawn(async move {
            if let Err(e) = curator_clone.start().await {
                tracing::error!("Curator daemon failed: {}", e);
            }
        });

        Ok(Self {
            config,
            working,
            retrieval,
            storage,
            skills,
            visual,
            curator,
            session_id,
        })
    }

    /// Get current session ID
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Process a user message through the full pipeline
    #[instrument(skip(self, user_input))]
    pub async fn process(&self, user_input: String) -> Result<PipelineResponse> {
        let pipeline_start = std::time::Instant::now();
        info!("Processing user input for session {}", self.session_id);

        // Stage 1: WMManager — Token budget check + compress/render
        let stage1_start = std::time::Instant::now();
        let working_state = self.stage1_wm_manager(&user_input).await?;
        let stage1 = StageResult {
            stage: PipelineStage::WmManager,
            success: true,
            duration_ms: stage1_start.elapsed().as_millis() as u64,
            metadata: serde_json::json!({ "budget_usage": working_state.budget.usage_ratio() }),
        };

        // Stage 2: SearchCoordinator — Planner → Semantic + Skill retrieval
        let stage2_start = std::time::Instant::now();
        let search_results = self.stage2_search_coordinator(&user_input, &working_state).await?;
        let stage2 = StageResult {
            stage: PipelineStage::SearchCoordinator,
            success: true,
            duration_ms: stage2_start.elapsed().as_millis() as u64,
            metadata: serde_json::json!({ "results_count": search_results.len() }),
        };

        // Stage 3: SynthesizerAgent — LLM-as-Judge scoring + context fusion
        let stage3_start = std::time::Instant::now();
        let background_context = self.stage3_synthesizer(&user_input, &search_results).await?;
        let stage3 = StageResult {
            stage: PipelineStage::SynthesizerAgent,
            success: true,
            duration_ms: stage3_start.elapsed().as_millis() as u64,
            metadata: serde_json::json!({ "context_length": background_context.len() }),
        };

        // Stage 4: ReasoningAgent — Final response generation
        let stage4_start = std::time::Instant::now();
        let response = self.stage4_reasoning(&user_input, &background_context).await?;
        let stage4 = StageResult {
            stage: PipelineStage::ReasoningAgent,
            success: true,
            duration_ms: stage4_start.elapsed().as_millis() as u64,
            metadata: serde_json::json!({ "response_length": response.len() }),
        };

        // Append assistant turn to working memory
        self.append_assistant_turn(&response).await?;

        // Stage 5: ConsolidatorAgent (background) — triggered automatically
        self.curator.trigger_consolidation(ConsolidationTrigger::TokenBudget);

        let total_duration = pipeline_start.elapsed().as_millis() as u64;
        info!("Pipeline complete in {}ms", total_duration);

        Ok(PipelineResponse {
            session_id: self.session_id,
            response,
            background_context,
            search_results,
            stages: vec![stage1, stage2, stage3, stage4],
            total_duration_ms: total_duration,
            token_usage: working_state.budget.current_tokens,
        })
    }

    /// Stage 1: WMManager — Token budget check + compress/render
    async fn stage1_wm_manager(&self, user_input: &str) -> Result<WorkingState> {
        let mut working = self.working.write().await;
        working.append_user(user_input.to_string())?;
        
        let (turns, compressed) = working.get_state();
        let budget = working.budget().clone();
        
        Ok(WorkingState {
            turns,
            compressed,
            budget,
        })
    }

    /// Stage 2: SearchCoordinator — Planner → Semantic + Skill retrieval
    async fn stage2_search_coordinator(
        &self,
        query: &str,
        working_state: &WorkingState,
    ) -> Result<Vec<crate::retrieval::SearchResult>> {
        // Build recent context from compressed summaries + raw recent turns
        let recent_context = self.build_recent_context(working_state);
        
        // Derive ActionState
        let action_state = ActionState {
            tentative_action: None, // Would be derived from query
            active_constraints: Vec::new(),
            failure_signals: Vec::new(),
            token_budget: working_state.budget.remaining(),
            recent_tools: Vec::new(),
        };

        // Build SearchPlan (in practice, LLM call with RETRIEVAL_PLANNING_SYSTEM)
        let plan = SearchPlan {
            mode: RetrievalMode::Compact,
            semantic_queries: vec![query.to_string()],
            tool_hints: self.extract_tool_hints(query),
            required_constraints: Vec::new(),
            tree_depth: self.config.retrieval.max_tree_depth,
            include_skills: true,
            include_visual: true,
        };

        // Execute retrieval
        let results = self.retrieval.retrieve(query, &action_state, &plan).await?;
        
        // Skill retrieval (mode-aware)
        let skill_mode = self.determine_skill_mode(query);
        let skill_results = self.skills.hyde_retrieve(query, skill_mode, 5).await;
        
        // Convert skill results to search results
        for skill in skill_results {
            // Would add to results
        }

        Ok(results)
    }

    /// Stage 3: SynthesizerAgent — LLM-as-Judge scoring + context fusion
    async fn stage3_synthesizer(
        &self,
        query: &str,
        search_results: &[crate::retrieval::SearchResult],
    ) -> Result<String> {
        // In practice: LLM call with SYNTHESIZER_PROMPT
        // Scores each fragment on 0-1 relevance, discards < 0.6
        // Fuses survivors into dense background_context
        
        let mut context_parts = Vec::new();
        
        for result in search_results {
            if result.score >= 0.6 {
                context_parts.push(format!(
                    "[{:?} score={:.2}] {}",
                    result.source, result.score, result.semantic_text
                ));
            }
        }
        
        Ok(context_parts.join("\n\n"))
    }

    /// Stage 4: ReasoningAgent — Final response generation
    async fn stage4_reasoning(
        &self,
        query: &str,
        background_context: &str,
    ) -> Result<String> {
        // In practice: LLM call with REASONING_PROMPT
        // Receives compressed_history, background_context, skill_reuse_plan
        // Generates final assistant reply
        
        // Placeholder response
        Ok(format!("Based on the context: {}\n\nFor '{}', I would...", 
                   background_context.chars().take(200).collect::<String>(), query))
    }

    /// Append assistant turn to working memory
    async fn append_assistant_turn(&self, response: &str) -> Result<()> {
        let mut working = self.working.write().await;
        working.append_assistant(response.to_string())?;
        
        // Store in semantic memory (would trigger consolidation)
        let record = MemoryRecord::new(
            crate::types::MemoryType::Event,
            format!("Assistant response: {}", response),
            response.to_string(),
            self.session_id,
        );
        self.storage.store_record(&record).await?;
        
        Ok(())
    }

    fn build_recent_context(&self, state: &WorkingState) -> String {
        let mut parts = Vec::new();
        
        if let Some(compressed) = &state.compressed {
            for anchor in &compressed.summaries {
                parts.push(format!("[Summary {}]: {}", anchor.turn_range.0, anchor.summary));
            }
        }
        
        for turn in &state.turns {
            parts.push(format!("{}: {}", turn.role as u8, turn.content));
        }
        
        parts.join("\n")
    }

    fn extract_tool_hints(&self, query: &str) -> Vec<String> {
        let tools = ["git", "docker", "kubectl", "aws", "gcp", "terraform", "ansible", "sql", "api", "database"];
        let query_lower = query.to_lowercase();
        tools.iter()
            .filter(|t| query_lower.contains(*t))
            .map(|s| s.to_string())
            .collect()
    }

    fn determine_skill_mode(&self, query: &str) -> SkillMode {
        if query.contains("how to") || query.contains("steps") || query.contains("run") {
            SkillMode::Action
        } else if query.contains("what") || query.contains("explain") || query.contains("describe") {
            SkillMode::Answer
        } else {
            SkillMode::Mixed
        }
    }

    /// Get pipeline status
    pub async fn status(&self) -> PipelineStatus {
        let working = self.working.read().await;
        let (turns, compressed) = working.get_state();
        let budget = working.budget();
        
        PipelineStatus {
            session_id: self.session_id,
            turn_count: turns.len(),
            compressed_summary_count: compressed.as_ref().map(|c| c.summaries.len()).unwrap_or(0),
            token_budget_usage: budget.usage_ratio(),
            is_warn: budget.is_warn(),
            is_blocked: budget.is_blocked(),
        }
    }

    /// Reset pipeline for new session
    pub async fn reset(&mut self) -> Result<()> {
        self.session_id = SessionId::new();
        let mut working = self.working.write().await;
        working.reset();
        Ok(())
    }
}

/// Working memory state snapshot
#[derive(Debug, Clone)]
pub struct WorkingState {
    pub turns: std::collections::VecDeque<crate::working::Turn>,
    pub compressed: Option<CompressedHistory>,
    pub budget: crate::types::TokenBudget,
}

/// Pipeline response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResponse {
    pub session_id: SessionId,
    pub response: String,
    pub background_context: String,
    pub search_results: Vec<crate::retrieval::SearchResult>,
    pub stages: Vec<StageResult>,
    pub total_duration_ms: u64,
    pub token_usage: usize,
}

/// Pipeline status for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatus {
    pub session_id: SessionId,
    pub turn_count: usize,
    pub compressed_summary_count: usize,
    pub token_budget_usage: f32,
    pub is_warn: bool,
    pub is_blocked: bool,
}