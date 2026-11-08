//! Retrieval Engine — Action-Aware Hierarchical Retrieval (from LycheeMem)

use crate::types::{
    ActionMetadata, ActionState, CompositeRecord, MemoryRecord, MemoryTree, MemoryType, RecordId,
    RetrievalMode, SearchPlan,
};
use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info};

/// Retrieval configuration
#[derive(Debug, Clone)]
pub struct RetrievalConfig {
    /// ANN pre-filter limit (top-k composites)
    pub ann_limit: usize,
    /// Similarity threshold for deduplication
    pub dedup_threshold: f32,
    /// Clustering threshold for composites
    pub cluster_threshold: f32,
    /// Maximum tree expansion depth
    pub max_tree_depth: usize,
    /// Reflection loop max rounds
    pub max_reflection_rounds: usize,
    /// Relevance threshold for SynthesizerAgent
    pub relevance_threshold: f32,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            ann_limit: 20,
            dedup_threshold: 0.85,
            cluster_threshold: 0.75,
            max_tree_depth: 3,
            max_reflection_rounds: 3,
            relevance_threshold: 0.6,
        }
    }
}

/// Search result with provenance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub record_id: RecordId,
    pub memory_type: MemoryType,
    pub semantic_text: String,
    pub score: f32,
    pub source: SearchSource,
    pub expanded: bool,
}

/// Source of the search result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSource {
    CompositeRelevance,
    TreeExpansion,
    SupplementaryFts,
    SupplementaryVector,
    SupplementaryEpisodic,
}

/// Retrieval engine implementing Action-Aware Hierarchical Retrieval
pub struct RetrievalEngine {
    config: RetrievalConfig,
    // In practice, these would be references to storage
    // For now, we'll use in-memory indices
    composite_index: DashMap<MemoryType, Vec<CompositeRecord>>,
    atomic_index: DashMap<MemoryType, Vec<MemoryRecord>>,
}

impl RetrievalEngine {
    pub fn new(config: RetrievalConfig) -> Self {
        Self {
            config,
            composite_index: DashMap::new(),
            atomic_index: DashMap::new(),
        }
    }

    /// Build indices from memory tree
    pub fn build_indices(&self, tree: &MemoryTree) {
        for (id, composite) in &tree.composites {
            self.composite_index
                .entry(composite.memory_type)
                .or_default()
                .push(composite.clone());
        }
        for (id, record) in &tree.atomic {
            self.atomic_index
                .entry(record.memory_type)
                .or_default()
                .push(record.clone());
        }
        info!("Built retrieval indices: {} composites, {} atomic records",
              tree.composites.len(), tree.atomic.len());
    }

    /// Main retrieval entry point — implements the 4-module pipeline
    pub async fn retrieve(
        &self,
        query: &str,
        action_state: &ActionState,
        plan: &SearchPlan,
    ) -> Result<Vec<SearchResult>> {
        let mut all_results = Vec::new();

        // Module 3: Action-Aware Hierarchical Retrieval
        // Step 1: ANN pre-filter to top-20 nearest CompositeRecords
        let candidate_composites = self.ann_pre_filter(query, plan).await?;

        // Step 2: Composite-level relevance judgement (single LLM call)
        let (relevant, needs_expansion) = self.composite_relevance_judgement(
            query, action_state, &candidate_composites, plan
        ).await?;

        // Add relevant composites to results
        for composite in relevant {
            all_results.push(SearchResult {
                record_id: composite.record_id.clone(),
                memory_type: composite.memory_type,
                semantic_text: composite.semantic_text.clone(),
                score: 1.0, // Would come from LLM judgement
                source: SearchSource::CompositeRelevance,
                expanded: false,
            });
        }

        // Step 3: Memory tree expansion for flagged composites
        let expanded_results = self.tree_expansion(&needs_expansion, plan.tree_depth).await?;
        all_results.extend(expanded_results);

        // Step 4: Reflection-based supplementary recall
        let supplementary = self.reflection_recall(query, action_state, &all_results, plan).await?;
        all_results.extend(supplementary);

        // Module 4: Candidate Aggregation and Context Enrichment
        // Rank by source tier and deduplicate
        let final_results = self.aggregate_and_rank(all_results, plan).await?;

        Ok(final_results)
    }

    /// Module 3.1: ANN pre-filter to top-k nearest CompositeRecords
    async fn ann_pre_filter(
        &self,
        query: &str,
        plan: &SearchPlan,
    ) -> Result<Vec<CompositeRecord>> {
        // In practice, this would query LanceDB with the query embedding
        // For now, return all composites of relevant types
        let mut candidates = Vec::new();
        
        for mt in MemoryType::all() {
            if let Some(composites) = self.composite_index.get(mt) {
                for composite in composites.iter() {
                    // Filter by tool hints if provided
                    if !plan.tool_hints.is_empty() {
                        let has_hint = composite.metadata.tool_tags.iter()
                            .any(|t| plan.tool_hints.iter().any(|h| t.contains(h)));
                        if !has_hint { continue; }
                    }
                    candidates.push(composite.clone());
                }
            }
        }

        // Sort by some heuristic (recency, usage) and take top-k
        candidates.sort_by(|a, b| {
            b.usage.retrieval_count.cmp(&a.usage.retrieval_count)
        });
        candidates.truncate(self.config.ann_limit);
        
        debug!("ANN pre-filter returned {} candidates", candidates.len());
        Ok(candidates)
    }

    /// Module 3.2: Composite-level relevance judgement (single LLM call)
    async fn composite_relevance_judgement(
        &self,
        query: &str,
        action_state: &ActionState,
        candidates: &[CompositeRecord],
        plan: &SearchPlan,
    ) -> Result<(Vec<CompositeRecord>, Vec<CompositeRecord>)> {
        // In practice, this is a single LLM call with all candidates
        // For now, use a simple heuristic: select composites with matching tool_tags
        let mut relevant = Vec::new();
        let mut needs_expansion = Vec::new();

        for composite in candidates {
            let mut score = 0.0;
            
            // Tool tag matching
            if !plan.tool_hints.is_empty() {
                for hint in &plan.tool_hints {
                    if composite.metadata.tool_tags.iter().any(|t| t.contains(hint)) {
                        score += 0.3;
                    }
                }
            }
            
            // Constraint matching
            if !plan.required_constraints.is_empty() {
                for constraint in &plan.required_constraints {
                    if composite.metadata.constraint_tags.iter().any(|c| c.contains(constraint)) {
                        score += 0.2;
                    }
                }
            }
            
            // Usage-based relevance
            if composite.usage.retrieval_count > 0 {
                score += 0.1;
            }
            
            if score > 0.3 {
                relevant.push(composite.clone());
            } else if score > 0.1 {
                needs_expansion.push(composite.clone());
            }
        }

        debug!("Relevance judgement: {} relevant, {} need expansion", relevant.len(), needs_expansion.len());
        Ok((relevant, needs_expansion))
    }

    /// Module 3.3: Tree expansion for flagged composites
    async fn tree_expansion(
        &self,
        composites: &[CompositeRecord],
        max_depth: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();
        let mut visited = HashSet::new();

        for composite in composites {
            let expanded = self.expand_composite(composite, max_depth, &mut visited).await?;
            results.extend(expanded);
        }

        Ok(results)
    }

    async fn expand_composite(
        &self,
        composite: &CompositeRecord,
        depth: usize,
        visited: &mut HashSet<RecordId>,
    ) -> Result<Vec<SearchResult>> {
        if depth == 0 || visited.contains(&composite.record_id) {
            return Ok(Vec::new());
        }
        visited.insert(composite.record_id.clone());

        let mut results = Vec::new();

        // Expand to child composites (level 2)
        for child_id in &composite.child_composite_ids_level2 {
            if let Some(child_composite) = self.find_composite(child_id) {
                results.push(SearchResult {
                    record_id: child_composite.record_id.clone(),
                    memory_type: child_composite.memory_type,
                    semantic_text: child_composite.semantic_text.clone(),
                    score: 0.8,
                    source: SearchSource::TreeExpansion,
                    expanded: true,
                });
                // Recurse - box the future to avoid infinite size
                let deeper = Box::pin(self.expand_composite(&child_composite, depth - 1, visited)).await?;
                results.extend(deeper);
            }
        }

        // Expand to source atomic records
        for source_id in &composite.source_record_ids {
            if let Some(record) = self.find_atomic(source_id) {
                results.push(SearchResult {
                    record_id: record.record_id.clone(),
                    memory_type: record.memory_type,
                    semantic_text: record.semantic_text.clone(),
                    score: 0.9,
                    source: SearchSource::TreeExpansion,
                    expanded: true,
                });
            }
        }

        Ok(results)
    }

    fn find_composite(&self, id: &RecordId) -> Option<CompositeRecord> {
        for entry in self.composite_index.iter() {
            for composite in entry.value() {
                if &composite.record_id == id {
                    return Some(composite.clone());
                }
            }
        }
        None
    }

    fn find_atomic(&self, id: &RecordId) -> Option<MemoryRecord> {
        for entry in self.atomic_index.iter() {
            for record in entry.value() {
                if &record.record_id == id {
                    return Some(record.clone());
                }
            }
        }
        None
    }

    /// Module 3.4: Reflection-based supplementary recall
    async fn reflection_recall(
        &self,
        query: &str,
        action_state: &ActionState,
        current_results: &[SearchResult],
        plan: &SearchPlan,
    ) -> Result<Vec<SearchResult>> {
        // Assess adequacy of current context
        let coverage_score = self.assess_coverage(query, &current_results.iter().collect::<Vec<_>>());
        
        if coverage_score > 0.8 {
            return Ok(Vec::new()); // Adequate coverage
        }

        let mut supplementary = Vec::new();
        let mut rounds = 0;

        while coverage_score <= 0.8 && rounds < self.config.max_reflection_rounds {
            rounds += 1;
            debug!("Reflection round {} (coverage: {:.2})", rounds, coverage_score);

            // Channel 1: FTS full-text search
            let fts_results = self.fts_recall(query, plan).await?;
            supplementary.extend(fts_results.into_iter().map(|r| SearchResult {
                record_id: r.record_id,
                memory_type: r.memory_type,
                semantic_text: r.semantic_text,
                score: 0.5,
                source: SearchSource::SupplementaryFts,
                expanded: false,
            }));

            // Channel 2: Vector search on semantic_text
            let vector_results = self.vector_recall(query, plan).await?;
            supplementary.extend(vector_results.into_iter().map(|r| SearchResult {
                record_id: r.record_id,
                memory_type: r.memory_type,
                semantic_text: r.semantic_text,
                score: 0.6,
                source: SearchSource::SupplementaryVector,
                expanded: false,
            }));

            // Channel 3: Vector search on normalized_text
            let norm_vector_results = self.normalized_vector_recall(query, plan).await?;
            supplementary.extend(norm_vector_results.into_iter().map(|r| SearchResult {
                record_id: r.record_id,
                memory_type: r.memory_type,
                semantic_text: r.semantic_text,
                score: 0.55,
                source: SearchSource::SupplementaryVector,
                expanded: false,
            }));

            // Channel 4: Raw episode turn recall
            let episodic_results = self.episodic_recall(query, action_state, plan).await?;
            supplementary.extend(episodic_results.into_iter().map(|r| SearchResult {
                record_id: r.record_id,
                memory_type: r.memory_type,
                semantic_text: r.semantic_text,
                score: 0.4,
                source: SearchSource::SupplementaryEpisodic,
                expanded: false,
            }));

            // Re-assess coverage
            let all_results: Vec<_> = current_results.iter().chain(supplementary.iter()).collect();
            let new_coverage = self.assess_coverage(query, &all_results);
            
            if new_coverage <= coverage_score + 0.05 {
                break; // Diminishing returns
            }
        }

        Ok(supplementary)
    }

    async fn fts_recall(&self, query: &str, plan: &SearchPlan) -> Result<Vec<MemoryRecord>> {
        // In practice, query SQLite FTS5
        Ok(Vec::new())
    }

    async fn vector_recall(&self, query: &str, plan: &SearchPlan) -> Result<Vec<MemoryRecord>> {
        // In practice, query LanceDB with query embedding
        Ok(Vec::new())
    }

    async fn normalized_vector_recall(&self, query: &str, plan: &SearchPlan) -> Result<Vec<MemoryRecord>> {
        Ok(Vec::new())
    }

    async fn episodic_recall(&self, query: &str, action_state: &ActionState, plan: &SearchPlan) -> Result<Vec<MemoryRecord>> {
        Ok(Vec::new())
    }

    fn assess_coverage(&self, query: &str, results: &[&SearchResult]) -> f32 {
        // Simple heuristic: ratio of unique tool_tags/constraint_tags covered
        let query_terms: HashSet<String> = query.split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();
        
        let mut covered = 0;
        let mut total = query_terms.len();
        
        if total == 0 {
            return 1.0;
        }

        for result in results {
            let text = result.semantic_text.to_lowercase();
            for term in &query_terms {
                if text.contains(term) {
                    covered += 1;
                }
            }
        }

        (covered as f32 / total as f32).min(1.0)
    }

    /// Module 4: Candidate Aggregation and Ranking
    async fn aggregate_and_rank(
        &self,
        mut results: Vec<SearchResult>,
        plan: &SearchPlan,
    ) -> Result<Vec<SearchResult>> {
        // Deduplicate by record_id, keeping highest score
        let mut deduped: HashMap<RecordId, SearchResult> = HashMap::new();
        for result in results.drain(..) {
            deduped.entry(result.record_id.clone())
                .and_modify(|existing| {
                    if result.score > existing.score {
                        *existing = result.clone();
                    }
                })
                .or_insert(result);
        }

        let mut final_results: Vec<_> = deduped.into_values().collect();

        // Rank by source tier priority:
        // 1. CompositeRelevance (highest)
        // 2. TreeExpansion
        // 3. SupplementaryVector
        // 4. SupplementaryFts
        // 5. SupplementaryEpisodic (lowest)
        final_results.sort_by(|a, b| {
            let tier_a = source_tier(a.source);
            let tier_b = source_tier(b.source);
            tier_b.cmp(&tier_a)
                .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Apply relevance threshold
        final_results.retain(|r| r.score >= self.config.relevance_threshold);

        // Top-k
        if final_results.len() > plan.tree_depth * 10 {
            final_results.truncate(plan.tree_depth * 10);
        }

        Ok(final_results)
    }
}

fn source_tier(source: SearchSource) -> u8 {
    match source {
        SearchSource::CompositeRelevance => 5,
        SearchSource::TreeExpansion => 4,
        SearchSource::SupplementaryVector => 3,
        SearchSource::SupplementaryFts => 2,
        SearchSource::SupplementaryEpisodic => 1,
    }
}