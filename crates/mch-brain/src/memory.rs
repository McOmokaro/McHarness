//! Memory types and tree structures

use std::collections::HashMap;

/// Memory tree structure
#[derive(Debug, Clone, Default)]
pub struct MemoryTree {
    /// Root composite records (level 1)
    pub roots: Vec<crate::types::RecordId>,
    /// Level 2 composites (abstractions over roots)
    pub level2: Vec<crate::types::RecordId>,
    /// All atomic records by ID
    pub atomic: HashMap<crate::types::RecordId, crate::types::MemoryRecord>,
    /// All composite records by ID
    pub composites: HashMap<crate::types::RecordId, crate::types::CompositeRecord>,
}

// Public re-exports from types
pub use crate::types::{MemoryRecord, CompositeRecord, MemoryType, RecordId, SessionId};

impl MemoryTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_records(&self) -> usize {
        self.atomic.len() + self.composites.len()
    }

    pub fn get_composite(&self, id: &RecordId) -> Option<&CompositeRecord> {
        self.composites.get(id)
    }

    pub fn get_atomic(&self, id: &RecordId) -> Option<&MemoryRecord> {
        self.atomic.get(id)
    }
}