//! Fusion: all skills ever, one core, addressed by domain.
//!
//! Type your problem — every skill for that domain is already armed.
//! One command reverse-engineers and merges newly released skills.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub domain: String,
    pub instructions: String,
    pub source_lineage: Option<String>,
}

pub struct Fusion {
    skills_by_domain: HashMap<String, Vec<Skill>>,
}

impl Fusion {
    pub fn new() -> Self {
        Self {
            skills_by_domain: HashMap::new(),
        }
    }

    /// Merge a skill into the core. New or known — it arms by domain.
    pub fn fuse(&mut self, skill: Skill) {
        self.skills_by_domain
            .entry(skill.domain.clone())
            .or_default()
            .push(skill);
    }

    /// Arm every skill for a domain. This is what "typing a problem" hits.
    pub fn arm(&self, domain: &str) -> Vec<&Skill> {
        self.skills_by_domain.get(domain).map(|v| v.iter().collect()).unwrap_or_default()
    }

    pub fn domains(&self) -> Vec<String> {
        self.skills_by_domain.keys().cloned().collect()
    }

    pub fn total(&self) -> usize {
        self.skills_by_domain.values().map(|v| v.len()).sum()
    }

    /// Merge a batch (e.g. a Fusion update release) in one step.
    pub fn fuse_batch(&mut self, skills: Vec<Skill>) {
        for s in skills {
            self.fuse(s);
        }
    }
}

pub fn load_skills_json(_json: &str) -> Result<Vec<Skill>> {
    // v0.0.1 stub: the Fusion update loader lands with the Ingestor.
    Ok(Vec::new())
}
