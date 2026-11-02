//! mch-memory — the three-strata memory, the skill-sealing engine, and the curator.
//!
//! Mined from Hermes Agent. The split is the design: declarative memory (facts, every turn),
//! procedural memory (skills, on demand), and the curator that keeps the skill library from
//! rotting into one-session-one-skill micro-entries.
//!
//! Strata:
//!   * `store`    — persistent, declarative memory (MEMORY.md / USER.md), frozen at session start
//!   * `skill`    — skill-sealing: experience becomes a reusable, ledgered, locked skill
//!   * `curator`  — the learning loop: active -> stale -> archived, never delete
//!   * `graph`    — learning made visible: skills + memory cards as one graph
//!   * `provider` — the pluggable external-memory contract

pub mod curator;
pub mod graph;
pub mod provider;
pub mod skill;
pub mod store;

pub use curator::{Curator, SkillRecord, SkillState, TransitionReport};
pub use graph::{LearningGraph, MemoryCard, SkillNode};
pub use provider::{MemoryProvider, RecallStatus};
pub use skill::{
    SkillError, SkillLedger, SkillOp, SkillStore, MAX_NAME_LENGTH, MAX_SKILL_BODY,
    SKILL_INDEX_DESC_LIMIT,
};
pub use store::{
    MemoryError, MemoryOp, MemoryStore, MemoryTarget, ENTRY_DELIMITER, MEMORY_CHARS, USER_CHARS,
};
