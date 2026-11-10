//! mch-brain — Native Rust Memory Graph for LLM Agents
//!
//! Forged from LycheeMem (HITsz-TMG/LycheeMem) and LycheeMemory (owoakuma/LycheeMemory).
//! Implements a four-store memory architecture with hierarchical consolidation,
//! action-aware retrieval, and MCP compatibility.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      mch-brain                              │
//! ├──────────────┬──────────────┬──────────────┬───────────────┤
//! │ Working Mem  │ Semantic Mem │ Procedural   │ Visual Mem    │
//! │ (Episodic)   │ (Typed Store)│ (Skills)     │ (Multimodal)  │
//! ├──────────────┼──────────────┼──────────────┼───────────────┤
//! │ Session turns│ 7 Record     │ Skill entries│ VLM captions  │
//! │ Summaries    │ types        │ HyDE retrieval│ CLIP vectors │
//! │ Token budget │ SQLite+LanceDB│ Usage stats  │ Ebbinghaus    │
//! │ (70/90%)     │ Hierarchy    │              │ forgetting    │
//! └──────────────┴──────────────┴──────────────┴───────────────┘
//!         │              │              │              │
//!         └──────────────┴──────────────┴──────────────┘
//!                          │
//!                    ┌─────▼─────┐
//!                    │  Curator  │  Background consolidation daemon
//!                    │  Daemon   │  Record Fusion → CompositeRecords
//!                    └───────────┘
//! ```
//!
//! # Pipeline (5 Agents)
//!
//! 1. **WMManager** — Token budget check + compress/render
//! 2. **SearchCoordinator** — Planner → Semantic + Skill retrieval
//! 3. **SynthesizerAgent** — LLM-as-Judge scoring + context fusion
//! 4. **ReasoningAgent** — Final response generation
//! 5. **ConsolidatorAgent** (background) — Compact consolidation + Skill extraction

pub mod cls;
pub mod curator;
pub mod memory;
pub mod pipeline;
pub mod retrieval;
pub mod skills;
pub mod storage;
pub mod types;
pub mod visual;
pub mod working;

pub use cls::{
    ClsConfig, DualProcessMemory, Episode, EpisodeKind, HeuristicRuleExtractor, MemoryEdge,
    ReplayDaemon, RuleExtractor, SemanticFact,
};
pub use curator::{CuratorDaemon, CuratorConfig};
pub use memory::{MemoryTree};
pub use pipeline::{BrainPipeline, PipelineConfig, PipelineStage};
pub use retrieval::{RetrievalEngine, RetrievalConfig};
pub use skills::{SkillStore, SkillEntry};
pub use storage::{BrainStorage, StorageConfig, SqliteStore};
pub use types::{MemoryRecord, CompositeRecord, MemoryType, RecordId, SessionId, TokenBudget, ConsolidationTrigger, SearchPlan, ActionState, RetrievalMode, ActionMetadata, ActionOutcome};
pub use visual::{VisualMemory, VisualRecord, VisualConfig};
pub use working::{WorkingMemory, WorkingConfig, CompressedHistory};