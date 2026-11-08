//! Visual Memory — Multimodal memory with VLM + CLIP embeddings

use crate::types::{RecordId, SessionId};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Scene type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SceneType {
    Screenshot,
    Chart,
    Photo,
    Document,
    Ui,
    Code,
    Other,
}

/// Visual memory record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualRecord {
    /// SHA256(image_hash + session_id + timestamp)
    pub record_id: RecordId,
    /// Natural-language description from VLM (primary retrieval anchor)
    pub caption: String,
    /// Scene classification
    pub scene_type: SceneType,
    /// Text embedding of caption (backward-compatible)
    #[serde(skip)]
    pub caption_embedding: Option<Vec<f32>>,
    /// CLIP visual embedding (enables cross-modal retrieval)
    #[serde(skip)]
    pub visual_embedding: Option<Vec<f32>>,
    /// 0.0-1.0, governs forgetting rate
    pub importance_score: f32,
    /// Content hash for deduplication
    pub image_hash: String,
    /// Local filesystem path
    pub image_path: PathBuf,
    /// Source session
    pub session_id: SessionId,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last retrieval timestamp
    pub last_retrieved: Option<DateTime<Utc>>,
    /// Retrieval count
    pub retrieval_count: u64,
}

/// Visual memory configuration
#[derive(Debug, Clone)]
pub struct VisualConfig {
    /// VLM model for understanding
    pub vlm_model: String,
    /// CLIP model for embeddings
    pub clip_model: String,
    /// Fast VLM variant settings
    pub fast_max_size: u32,
    pub fast_jpeg_quality: u8,
    pub fast_timeout_secs: u64,
    /// Forgetting curve parameters
    pub base_half_life_days: f32,
    pub max_ttl_days: u32,
    /// Cache sizes
    pub extractor_cache_size: usize,
    pub embedder_cache_size: usize,
}

impl Default for VisualConfig {
    fn default() -> Self {
        Self {
            vlm_model: "qwen-vl-max".to_string(),
            clip_model: "clip-vit-base-patch32".to_string(),
            fast_max_size: 512,
            fast_jpeg_quality: 75,
            fast_timeout_secs: 15,
            base_half_life_days: 7.0,
            max_ttl_days: 90,
            extractor_cache_size: 128,
            embedder_cache_size: 256,
        }
    }
}

/// Visual memory store
pub struct VisualMemory {
    config: VisualConfig,
    records: Arc<RwLock<HashMap<RecordId, VisualRecord>>>,
    session_index: Arc<RwLock<HashMap<SessionId, Vec<RecordId>>>>,
    // Caches
    extractor_cache: Arc<RwLock<lru::LruCache<String, VisualExtraction>>>,
    embedder_cache: Arc<RwLock<lru::LruCache<String, Vec<f32>>>>,
}

/// VLM extraction result
#[derive(Debug, Clone)]
struct VisualExtraction {
    caption: String,
    entities: Vec<String>,
    scene_type: SceneType,
    importance_score: f32,
}

impl VisualMemory {
    pub fn new(config: VisualConfig) -> Self {
        Self {
            config,
            records: Arc::new(RwLock::new(HashMap::new())),
            session_index: Arc::new(RwLock::new(HashMap::new())),
            extractor_cache: Arc::new(RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(128).unwrap(),
            ))),
            embedder_cache: Arc::new(RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(256).unwrap(),
            ))),
        }
    }

    /// Add a visual record (from VLM understanding)
    pub async fn add_record(
        &self,
        image_path: PathBuf,
        image_hash: String,
        session_id: SessionId,
        caption: String,
        scene_type: SceneType,
        importance_score: f32,
        caption_embedding: Option<Vec<f32>>,
        visual_embedding: Option<Vec<f32>>,
    ) -> Result<VisualRecord> {
        let record_id = RecordId::from_content(&format!("{}{}{}", image_hash, session_id, Utc::now().timestamp()));
        
        let record = VisualRecord {
            record_id: record_id.clone(),
            caption,
            scene_type,
            caption_embedding,
            visual_embedding,
            importance_score,
            image_hash,
            image_path,
            session_id,
            created_at: Utc::now(),
            last_retrieved: None,
            retrieval_count: 0,
        };

        // Store in memory
        self.records.write().await.insert(record_id.clone(), record.clone());
        
        // Update session index
        self.session_index
            .write()
            .await
            .entry(session_id)
            .or_default()
            .push(record_id);

        info!("Added visual record: {} ({:?})", record.record_id, record.scene_type);
        Ok(record)
    }

    /// Get record by ID
    pub async fn get_record(&self, id: &RecordId) -> Option<VisualRecord> {
        self.records.read().await.get(id).cloned()
    }

    /// Text → Visual retrieval (dual-channel: caption + CLIP)
    pub async fn retrieve_text_to_visual(&self, query: &str, top_k: usize) -> Vec<VisualRecord> {
        // In practice:
        // 1. Embed query text → caption vector
        // 2. Embed query image (if provided) → CLIP visual vector
        // 3. Score fusion: α * caption_sim + (1-α) * visual_sim
        // 4. Top-k selection
        
        let records = self.records.read().await;
        let mut results: Vec<(VisualRecord, f32)> = records.values()
            .map(|r| {
                let score = self.text_similarity(query, &r.caption);
                (r.clone(), score)
            })
            .filter(|(_, score)| *score > 0.3)
            .collect();
        
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(top_k);
        
        // Update retrieval stats
        for (record, _) in &results {
            self.update_retrieval(&record.record_id).await;
        }
        
        results.into_iter().map(|(r, _)| r).collect()
    }

    /// Image → Similar images (CLIP visual ANN)
    pub async fn retrieve_similar_images(&self, query_image_hash: &str, top_k: usize) -> Vec<VisualRecord> {
        // In practice: CLIP visual embedding ANN search
        let records = self.records.read().await;
        let mut results: Vec<VisualRecord> = records.values()
            .filter(|r| r.image_hash != query_image_hash)
            .take(top_k)
            .cloned()
            .collect();
        
        for record in &results {
            self.update_retrieval(&record.record_id).await;
        }
        
        results
    }

    /// Session-level visual memory
    pub async fn get_session_visuals(&self, session_id: SessionId) -> Vec<VisualRecord> {
        let index = self.session_index.read().await;
        if let Some(ids) = index.get(&session_id) {
            let records = self.records.read().await;
            ids.iter().filter_map(|id| records.get(id).cloned()).collect()
        } else {
            Vec::new()
        }
    }

    /// Visual Forgetter — Ebbinghaus-inspired decay
    pub async fn apply_forgetting(&self) -> usize {
        let mut records = self.records.write().await;
        let now = Utc::now();
        let mut expired = 0;
        
        records.retain(|_, record| {
            let days_since_retrieval = if let Some(last) = record.last_retrieved {
                (now - last).num_days() as f32
            } else {
                (now - record.created_at).num_days() as f32
            };
            
            // Effective half-life scales with importance
            let half_life = self.config.base_half_life_days * (1.0 + 3.0 * record.importance_score);
            let decay = 0.5_f32.powf(days_since_retrieval / half_life);
            
            // Each retrieval adds up to +30% decay resistance
            let retrieval_boost = 1.0 + 0.3 * (record.retrieval_count as f32).min(10.0);
            let effective_decay = decay * retrieval_boost;
            
            if effective_decay < 0.1 || days_since_retrieval > self.config.max_ttl_days as f32 {
                expired += 1;
                false // Remove
            } else {
                true // Keep
            }
        });
        
        if expired > 0 {
            info!("Visual forgetter expired {} records", expired);
        }
        expired
    }

    async fn update_retrieval(&self, id: &RecordId) {
        let mut records = self.records.write().await;
        if let Some(record) = records.get_mut(id) {
            record.retrieval_count += 1;
            record.last_retrieved = Some(Utc::now());
        }
    }

    fn text_similarity(&self, query: &str, caption: &str) -> f32 {
        let query_lower = query.to_lowercase();
        let caption_lower = caption.to_lowercase();
        let words_q: std::collections::HashSet<_> = query_lower.split_whitespace().collect();
        let words_c: std::collections::HashSet<_> = caption_lower.split_whitespace().collect();
        let intersection = words_q.intersection(&words_c).count();
        let union = words_q.union(&words_c).count();
        if union == 0 { 0.0 } else { intersection as f32 / union as f32 }
    }

    /// Fast extraction variant (compressed images, shorter timeout)
    pub async fn extract_fast(&self, image_path: &PathBuf) -> Result<VisualExtraction> {
        // Check cache first
        let cache_key = format!("fast:{}:{}", image_path.display(), std::fs::metadata(image_path)?.modified()?.elapsed()?.as_secs());
        
        if let Some(cached) = self.extractor_cache.write().await.get(&cache_key) {
            return Ok(cached.clone());
        }

        // In practice: call VLM with compressed image
        // For now, return placeholder
        let extraction = VisualExtraction {
            caption: "Fast extraction placeholder".to_string(),
            entities: Vec::new(),
            scene_type: SceneType::Screenshot,
            importance_score: 0.5,
        };
        
        self.extractor_cache.write().await.put(cache_key, extraction.clone());
        Ok(extraction)
    }

    /// Standard extraction (full quality)
    pub async fn extract_standard(&self, image_path: &PathBuf) -> Result<VisualExtraction> {
        // In practice: call VLM with full image
        Ok(VisualExtraction {
            caption: "Standard extraction placeholder".to_string(),
            entities: Vec::new(),
            scene_type: SceneType::Screenshot,
            importance_score: 0.7,
        })
    }
}