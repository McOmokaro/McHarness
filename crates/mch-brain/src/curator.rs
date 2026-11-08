//! Curator Daemon — Background Consolidation (from LycheeMem)

use crate::memory::MemoryTree;
use crate::retrieval::{RetrievalEngine, RetrievalConfig};
use crate::skills::{SkillStore, SkillEntry};
use crate::storage::{BrainStorage, UnifiedStore};
use crate::types::{MemoryRecord, CompositeRecord, MemoryType, RecordId, SessionId, ConsolidationTrigger};
use crate::visual::VisualMemory;
use crate::working::{WorkingMemory, CompressedHistory};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

/// Curator configuration
#[derive(Debug, Clone)]
pub struct CuratorConfig {
    /// Consolidation interval (seconds)
    pub consolidation_interval_secs: u64,
    /// Forgetting interval (seconds)
    pub forgetting_interval_secs: u64,
    /// Max records per consolidation batch
    pub max_batch_size: usize,
    /// Enable skill extraction
    pub extract_skills: bool,
    /// Enable visual forgetting
    pub visual_forgetting: bool,
}

impl Default for CuratorConfig {
    fn default() -> Self {
        Self {
            consolidation_interval_secs: 300, // 5 minutes
            forgetting_interval_secs: 3600,   // 1 hour
            max_batch_size: 100,
            extract_skills: true,
            visual_forgetting: true,
        }
    }
}

/// Consolidation result
#[derive(Debug, Clone)]
pub struct ConsolidationResult {
    pub trigger: ConsolidationTrigger,
    pub session_id: SessionId,
    pub entities_added: usize,
    pub skills_added: usize,
    pub facts_added: usize,
    pub composites_created: usize,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Curator daemon for background memory maintenance
pub struct CuratorDaemon {
    config: CuratorConfig,
    storage: Arc<UnifiedStore>,
    retrieval: Arc<RetrievalEngine>,
    skills: Arc<SkillStore>,
    visual: Arc<VisualMemory>,
    working: Arc<RwLock<WorkingMemory>>,
    consolidation_tx: tokio::sync::mpsc::UnboundedSender<ConsolidationTrigger>,
    consolidation_rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<ConsolidationTrigger>>>,
}

impl CuratorDaemon {
    pub fn new(
        config: CuratorConfig,
        storage: Arc<UnifiedStore>,
        retrieval: Arc<RetrievalEngine>,
        skills: Arc<SkillStore>,
        visual: Arc<VisualMemory>,
        working: Arc<RwLock<WorkingMemory>>,
    ) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            config,
            storage,
            retrieval,
            skills,
            visual,
            working,
            consolidation_tx: tx,
            consolidation_rx: Arc::new(tokio::sync::Mutex::new(rx)),
        }
    }

    /// Start the curator daemon
    pub async fn start(&self) -> Result<()> {
        let consolidation_interval = Duration::from_secs(self.config.consolidation_interval_secs);
        let forgetting_interval = Duration::from_secs(self.config.forgetting_interval_secs);
        
        let mut consolidation_timer = interval(consolidation_interval);
        let mut forgetting_timer = interval(forgetting_interval);

        info!("Curator daemon started (consolidation: {}s, forgetting: {}s)", 
              self.config.consolidation_interval_secs, self.config.forgetting_interval_secs);

        loop {
            tokio::select! {
                _ = consolidation_timer.tick() => {
                    if let Err(e) = self.run_consolidation(ConsolidationTrigger::Periodic).await {
                        warn!("Periodic consolidation failed: {}", e);
                    }
                }
                _ = forgetting_timer.tick() => {
                    if let Err(e) = self.run_forgetting().await {
                        warn!("Forgetting pass failed: {}", e);
                    }
                }
                trigger = async {
                    let mut rx = self.consolidation_rx.lock().await;
                    rx.recv().await
                } => {
                    if let Some(trigger) = trigger {
                        if let Err(e) = self.run_consolidation(trigger).await {
                            warn!("Triggered consolidation failed: {}", e);
                        }
                    }
                }
            }
        }
    }

    /// Trigger consolidation for a session
    pub fn trigger_consolidation(&self, trigger: ConsolidationTrigger) {
        let _ = self.consolidation_tx.send(trigger);
    }

    /// Run consolidation (Module 2 of LycheeMem pipeline)
    async fn run_consolidation(&self, trigger: ConsolidationTrigger) -> Result<ConsolidationResult> {
        let start = std::time::Instant::now();
        info!("Starting consolidation (trigger: {:?})", trigger);

        // Get current memory tree
        let tree = self.storage.get_tree().await?;
        
        // Update retrieval indices
        self.retrieval.build_indices(&tree);

        let mut entities_added = 0;
        let mut skills_added = 0;
        let mut facts_added = 0;
        let mut composites_created = 0;

        // Process each session's recent turns
        let atomic_records: Vec<_> = tree.atomic.values().cloned().collect();
        
        // Group by session
        let mut by_session: HashMap<SessionId, Vec<MemoryRecord>> = HashMap::new();
        for record in atomic_records {
            by_session.entry(record.session_id).or_default().push(record);
        }

        // Compact consolidation for each session
        for (session_id, records) in by_session {
            if records.is_empty() { continue; }
            
            // In practice: call CompactSemanticEngine.ingest_conversation()
            // which does: typed extraction → decontextualization → action metadata annotation
            
            // Deduplication (cosine > 0.85)
            let deduped = self.deduplicate_records(&records)?;
            entities_added += deduped.len();
            
            // Count facts
            facts_added += deduped.iter().filter(|r| r.memory_type == MemoryType::Fact).count();
            
            // Clustering (cosine > 0.75) → CompositeRecord construction
            let composites = self.cluster_and_fuse(deduped)?;
            composites_created += composites.len();
            
            // Store composites
            for composite in composites {
                self.storage.store_composite(&composite).await?;
            }

            // Skill extraction
            if self.config.extract_skills {
                let working = self.working.read().await;
                let (turns, _) = working.get_state();
                let turns_slice: Vec<_> = turns.iter().cloned().collect();
                let new_skills = self.skills.extract_skills_from_conversation(&turns_slice).await;
                for skill in new_skills {
                    self.skills.add_skill(skill).await?;
                    skills_added += 1;
                }
            }
        }

        let result = ConsolidationResult {
            trigger,
            session_id: SessionId::new(), // Would be specific session
            entities_added,
            skills_added,
            facts_added,
            composites_created,
            duration_ms: start.elapsed().as_millis() as u64,
            timestamp: Utc::now(),
        };

        info!("Consolidation complete: {} entities, {} skills, {} facts, {} composites ({}ms)",
              entities_added, skills_added, facts_added, composites_created, result.duration_ms);
        
        Ok(result)
    }

    /// Deduplication pass (cosine similarity > 0.85)
    fn deduplicate_records(&self, records: &[MemoryRecord]) -> Result<Vec<MemoryRecord>> {
        let mut kept: Vec<MemoryRecord> = Vec::new();
        let mut skipped = HashMap::new();
        
        for record in records {
            let mut is_duplicate = false;
            for existing in &kept {
                if record.memory_type == existing.memory_type {
                    if let (Some(e1), Some(e2)) = (&record.embedding, &existing.embedding) {
                        if cosine_similarity(e1, e2) > 0.85 {
                            // Soft-expire: mark as duplicate of existing
                            skipped.insert(record.record_id.clone(), existing.record_id.clone());
                            is_duplicate = true;
                            break;
                        }
                    }
                }
            }
            if !is_duplicate {
                kept.push(record.clone());
            }
        }
        
        debug!("Deduplication: {} input → {} kept, {} soft-expired", records.len(), kept.len(), skipped.len());
        Ok(kept)
    }

    /// Clustering pass (cosine > 0.75) + CompositeRecord construction
    fn cluster_and_fuse(&self, records: Vec<MemoryRecord>) -> Result<Vec<CompositeRecord>> {
        // Union-Find for connected components
        let n = records.len();
        let mut parent = (0..n).collect::<Vec<_>>();
        
        fn find(parent: &mut [usize], x: usize) -> usize {
            if parent[x] != x {
                parent[x] = find(parent, parent[x]);
            }
            parent[x]
        }
        
        fn union(parent: &mut [usize], x: usize, y: usize) {
            let px = find(parent, x);
            let py = find(parent, y);
            if px != py {
                parent[px] = py;
            }
        }
        
        // Build similarity graph
        for i in 0..n {
            for j in (i+1)..n {
                if records[i].memory_type == records[j].memory_type {
                    if let (Some(e1), Some(e2)) = (&records[i].embedding, &records[j].embedding) {
                        if cosine_similarity(e1, e2) > 0.75 {
                            union(&mut parent, i, j);
                        }
                    }
                }
            }
        }
        
        // Group by component
        let mut components: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            components.entry(root).or_default().push(i);
        }
        
        // Build composites from components with at least one new record
        let mut composites = Vec::new();
        for (_, indices) in components {
            if indices.len() < 2 { continue; }
            
            // Representative = highest confidence / most recent
            let rep_idx = indices.iter()
                .max_by_key(|&&i| (records[i].metadata.confidence * 1000.0) as u64 + records[i].created_at.timestamp() as u64)
                .copied()
                .unwrap_or(indices[0]);
            
            let mut composite = CompositeRecord {
                record_id: RecordId::from_content(&records[rep_idx].semantic_text),
                memory_type: records[rep_idx].memory_type,
                semantic_text: records[rep_idx].semantic_text.clone(),
                metadata: merge_metadata(&indices.iter().map(|&i| &records[i].metadata).collect::<Vec<_>>()),
                usage: merge_usage(&indices.iter().map(|&i| &records[i].usage).collect::<Vec<_>>()),
                session_id: records[rep_idx].session_id,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                child_composite_ids: Vec::new(),
                source_record_ids: indices.iter().map(|&i| records[i].record_id.clone()).collect(),
                child_composite_ids_level2: Vec::new(),
                embedding: records[rep_idx].embedding.clone(),
            };
            
            composites.push(composite);
        }
        
        Ok(composites)
    }

    /// Run forgetting pass (visual + semantic)
    async fn run_forgetting(&self) -> Result<()> {
        info!("Running forgetting pass");
        
        // Visual forgetting (Ebbinghaus curve)
        if self.config.visual_forgetting {
            let expired = self.visual.apply_forgetting().await;
            info!("Visual forgetting: {} records expired", expired);
        }
        
        // Semantic forgetting could be implemented here
        // (e.g., decay usage stats, soft-expire old records)
        
        Ok(())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() { return 0.0; }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot / (norm_a * norm_b) }
}

fn merge_metadata(metadatas: &[&crate::types::ActionMetadata]) -> crate::types::ActionMetadata {
    let mut merged = crate::types::ActionMetadata::default();
    for m in metadatas {
        merged.tool_tags.extend(m.tool_tags.clone());
        merged.constraint_tags.extend(m.constraint_tags.clone());
        merged.failure_tags.extend(m.failure_tags.clone());
        merged.affordance_tags.extend(m.affordance_tags.clone());
        merged.confidence = merged.confidence.max(m.confidence);
    }
    // Deduplicate
    merged.tool_tags.sort(); merged.tool_tags.dedup();
    merged.constraint_tags.sort(); merged.constraint_tags.dedup();
    merged.failure_tags.sort(); merged.failure_tags.dedup();
    merged.affordance_tags.sort(); merged.affordance_tags.dedup();
    merged
}

fn merge_usage(usages: &[&crate::types::UsageStats]) -> crate::types::UsageStats {
    let mut merged = crate::types::UsageStats::default();
    for u in usages {
        merged.retrieval_count += u.retrieval_count;
        merged.action_success_count += u.action_success_count;
        merged.action_failure_count += u.action_failure_count;
        merged.last_retrieved = merged.last_retrieved.max(u.last_retrieved);
    }
    merged
}