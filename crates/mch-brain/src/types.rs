//! Core type definitions for mch-brain

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique session identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SessionId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Unique record identifier (SHA256 of normalized content)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordId(pub String);

impl RecordId {
    pub fn from_content(content: &str) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = hasher.finalize();
        Self(hex::encode(hash))
    }
}

impl std::fmt::Display for RecordId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Seven semantic memory types (from LycheeMem)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// Objective facts about the user, environment, or world
    Fact,
    /// User preferences (style, habits, likes/dislikes)
    Preference,
    /// Specific events that have occurred
    Event,
    /// Conditions that must be respected
    Constraint,
    /// Reusable step-by-step procedures / methods
    Procedure,
    /// Previously failed action paths and their causes
    FailurePattern,
    /// Capabilities and applicable scenarios of tools/APIs
    ToolAffordance,
}

impl MemoryType {
    pub fn all() -> &'static [MemoryType] {
        &[
            MemoryType::Fact,
            MemoryType::Preference,
            MemoryType::Event,
            MemoryType::Constraint,
            MemoryType::Procedure,
            MemoryType::FailurePattern,
            MemoryType::ToolAffordance,
        ]
    }
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::Fact => write!(f, "fact"),
            MemoryType::Preference => write!(f, "preference"),
            MemoryType::Event => write!(f, "event"),
            MemoryType::Constraint => write!(f, "constraint"),
            MemoryType::Procedure => write!(f, "procedure"),
            MemoryType::FailurePattern => write!(f, "failure_pattern"),
            MemoryType::ToolAffordance => write!(f, "tool_affordance"),
        }
    }
}

/// Action-facing metadata for MemoryRecords
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionMetadata {
    /// Tool names this record relates to
    pub tool_tags: Vec<String>,
    /// Constraint categories this record enforces
    pub constraint_tags: Vec<String>,
    /// Failure pattern categories
    pub failure_tags: Vec<String>,
    /// Tool affordance categories
    pub affordance_tags: Vec<String>,
    /// Confidence score (0.0-1.0)
    pub confidence: f32,
    /// Source turn indices
    pub source_turns: Vec<usize>,
}

/// Usage statistics for RL-ready signals
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStats {
    /// Times retrieved in search
    pub retrieval_count: u64,
    /// Times used in successful action
    pub action_success_count: u64,
    /// Times used in failed action
    pub action_failure_count: u64,
    /// Last retrieval timestamp
    pub last_retrieved: Option<DateTime<Utc>>,
    /// Last action outcome (success/failure/correction)
    pub last_outcome: Option<ActionOutcome>,
}

/// Action outcome feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOutcome {
    Success,
    Failure,
    Correction,
}

/// Core memory record — the atomic unit of semantic memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    /// SHA256(normalized_text) — naturally idempotent
    pub record_id: RecordId,
    /// Semantic category
    pub memory_type: MemoryType,
    /// Decontextualized, self-contained text
    pub normalized_text: String,
    /// Human-readable summary (for display)
    pub semantic_text: String,
    /// Action-facing metadata
    pub metadata: ActionMetadata,
    /// Usage statistics for RL
    pub usage: UsageStats,
    /// Source session
    pub session_id: SessionId,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Child composite IDs (for hierarchy)
    pub child_composite_ids: Vec<RecordId>,
    /// Source record IDs (for traceability)
    pub source_record_ids: Vec<RecordId>,
    /// Vector embedding (LanceDB)
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
}

impl MemoryRecord {
    pub fn new(
        memory_type: MemoryType,
        normalized_text: String,
        semantic_text: String,
        session_id: SessionId,
    ) -> Self {
        let record_id = RecordId::from_content(&normalized_text);
        let now = Utc::now();
        Self {
            record_id,
            memory_type,
            normalized_text,
            semantic_text,
            metadata: ActionMetadata::default(),
            usage: UsageStats::default(),
            session_id,
            created_at: now,
            updated_at: now,
            child_composite_ids: Vec::new(),
            source_record_ids: Vec::new(),
            embedding: None,
        }
    }

    /// Check if this record is a near-duplicate of another (cosine similarity > 0.85)
    pub fn is_near_duplicate(&self, other: &MemoryRecord, threshold: f32) -> bool {
        if self.memory_type != other.memory_type {
            return false;
        }
        if let (Some(e1), Some(e2)) = (&self.embedding, &other.embedding) {
            cosine_similarity(e1, e2) > threshold
        } else {
            false
        }
    }
}

/// Composite record — fused from multiple MemoryRecords
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeRecord {
    /// SHA256 of representative semantic_text
    pub record_id: RecordId,
    /// Semantic category (same for all fused records)
    pub memory_type: MemoryType,
    /// Representative text (from highest confidence record)
    pub semantic_text: String,
    /// Merged action metadata
    pub metadata: ActionMetadata,
    /// Aggregated usage stats
    pub usage: UsageStats,
    /// Source session (representative)
    pub session_id: SessionId,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Child composite IDs (hierarchy)
    pub child_composite_ids: Vec<RecordId>,
    /// Source atomic record IDs
    pub source_record_ids: Vec<RecordId>,
    /// Child composite IDs (next level up)
    pub child_composite_ids_level2: Vec<RecordId>,
    /// Vector embedding
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
}

/// Hierarchical memory tree structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryTree {
    /// Root composite records (level 1)
    pub roots: Vec<RecordId>,
    /// Level 2 composites (abstractions over roots)
    pub level2: Vec<RecordId>,
    /// All atomic records by ID
    pub atomic: HashMap<RecordId, MemoryRecord>,
    /// All composite records by ID
    pub composites: HashMap<RecordId, CompositeRecord>,
}

/// Token budget with dual thresholds (70% warn, 90% block)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Maximum tokens allowed
    pub max_tokens: usize,
    /// Warn threshold (fraction of max)
    pub warn_threshold: f32,
    /// Block threshold (fraction of max)
    pub block_threshold: f32,
    /// Current token count
    pub current_tokens: usize,
}

impl TokenBudget {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            warn_threshold: 0.7,
            block_threshold: 0.9,
            current_tokens: 0,
        }
    }

    pub fn usage_ratio(&self) -> f32 {
        self.current_tokens as f32 / self.max_tokens as f32
    }

    pub fn is_warn(&self) -> bool {
        self.usage_ratio() >= self.warn_threshold
    }

    pub fn is_blocked(&self) -> bool {
        self.usage_ratio() >= self.block_threshold
    }

    pub fn remaining(&self) -> usize {
        self.max_tokens.saturating_sub(self.current_tokens)
    }
}

/// Consolidation trigger types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationTrigger {
    /// Automatic on token budget threshold
    TokenBudget,
    /// Session boundary (/new, /reset, /stop)
    SessionBoundary,
    /// Strong long-term knowledge signal
    KnowledgeSignal,
    /// Manual API call
    Manual,
    /// Background periodic
    Periodic,
}

/// Search plan from SearchCoordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPlan {
    /// Retrieval mode
    pub mode: RetrievalMode,
    /// Semantic queries to execute
    pub semantic_queries: Vec<String>,
    /// Tool hints for retrieval
    pub tool_hints: Vec<String>,
    /// Required constraints
    pub required_constraints: Vec<String>,
    /// Tree traversal depth
    pub tree_depth: usize,
    /// Whether to include skills
    pub include_skills: bool,
    /// Whether to include visual memory
    pub include_visual: bool,
}

/// Retrieval mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetrievalMode {
    /// Compact one-shot recall
    Compact,
    /// Full retrieval for debugging
    Full,
    /// Action-oriented (tool-focused)
    Action,
    /// Mixed semantic + action
    Mixed,
}

/// Action state for retrieval conditioning
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionState {
    /// Current tentative action
    pub tentative_action: Option<String>,
    /// Recent constraints
    pub active_constraints: Vec<String>,
    /// Recent failure signals
    pub failure_signals: Vec<String>,
    /// Token budget remaining
    pub token_budget: usize,
    /// Recent tool use
    pub recent_tools: Vec<String>,
}

/// Cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}