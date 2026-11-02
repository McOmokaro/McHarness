//! Learning made visible — skills and memory cards as one graph.
//!
//! Mined from Hermes Agent's `agent/learning_graph.py`. The graph answers "what has this
//! agent actually learned" without guessing: a skill is a learning milestone only when the
//! agent created it, the user taught it, or it has been used. Memory-chunk nodes carry a
//! fingerprint of their text, so an edit does not collide with a neighbour.

use serde::{Deserialize, Serialize};

use crate::curator::has_learning_signal;

/// A learned skill as a graph node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillNode {
    pub name: String,
    pub category: String,
    pub created_by: Option<String>,
    pub use_count: u64,
    pub related: Vec<String>,
}

/// A `MEMORY.md` / `USER.md` entry as a graph node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCard {
    pub source: String,
    pub title: String,
    pub body: String,
    /// Short stable digest of the entry's text — identity, not position.
    pub fingerprint: String,
}

impl MemoryCard {
    pub fn new(source: impl Into<String>, body: impl Into<String>) -> Self {
        let body = body.into();
        let title: String = body
            .lines()
            .next()
            .unwrap_or("")
            .trim_start_matches('#')
            .trim()
            .chars()
            .take(80)
            .collect();
        let fingerprint = fingerprint(&body);
        Self {
            source: source.into(),
            title,
            body,
            fingerprint,
        }
    }

    pub fn node_id(&self, index: usize) -> String {
        format!("memory:{}:{}:{}", self.source, index, self.fingerprint)
    }
}

/// Short stable digest of an entry's text. FNV-1a; swap for SHA-256 if a dependency is wanted.
pub fn fingerprint(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.trim().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:012x}")
}

/// One edge (undirected; stored normalized).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
}

/// The full learning graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LearningGraph {
    pub skills: Vec<SkillNode>,
    pub memory: Vec<MemoryCard>,
    pub edges: Vec<Edge>,
}

impl LearningGraph {
    /// Build the graph from skills with a real learning signal plus memory cards.
    /// Skill-skill edges come from declared `related`; memory-skill edges from lexical overlap.
    pub fn build(skills: Vec<SkillNode>, memory: Vec<MemoryCard>) -> Self {
        let learned: Vec<SkillNode> = skills
            .into_iter()
            .filter(|s| has_learning_signal(s.created_by.as_deref(), s.use_count))
            .collect();

        let mut edges = Vec::new();

        // Declared related_skills, deduped, both endpoints present.
        let names: std::collections::HashSet<&str> =
            learned.iter().map(|s| s.name.as_str()).collect();
        let mut seen = std::collections::HashSet::new();
        for node in &learned {
            for target in &node.related {
                if !names.contains(target.as_str()) || target == &node.name {
                    continue;
                }
                let (a, b) = if node.name < *target {
                    (node.name.clone(), target.clone())
                } else {
                    (target.clone(), node.name.clone())
                };
                if seen.insert((a.clone(), b.clone())) {
                    edges.push(Edge { source: a, target: b });
                }
            }
        }

        // Memory -> skill lexical overlap: top few by shared tokens (name hit weighs heavily).
        for (index, card) in memory.iter().enumerate() {
            let text = format!("{} {}", card.title, card.body).to_lowercase();
            let text_tokens = tokenize(&text);
            let mut scored: Vec<(usize, &str)> = learned
                .iter()
                .filter_map(|s| {
                    let name_lower = s.name.to_lowercase();
                    let name_hit = if text.contains(&name_lower) { 6 } else { 0 };
                    let overlap = tokenize(&name_lower)
                        .intersection(&text_tokens)
                        .count();
                    let score = name_hit + overlap;
                    (score > 0).then_some((score, s.name.as_str()))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
            for (_, name) in scored.into_iter().take(4) {
                edges.push(Edge {
                    source: card.node_id(index),
                    target: name.to_string(),
                });
            }
        }

        Self {
            skills: learned,
            memory,
            edges,
        }
    }

    /// Count of skills with a learning signal.
    pub fn learned_skill_count(&self) -> usize {
        self.skills.len()
    }
}

fn tokenize(text: &str) -> std::collections::HashSet<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(|t| t.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_configured_skill_is_not_learned() {
        let skills = vec![SkillNode {
            name: "mounted".into(),
            category: "x".into(),
            created_by: None,
            use_count: 0,
            related: vec![],
        }];
        let graph = LearningGraph::build(skills, vec![]);
        assert_eq!(graph.learned_skill_count(), 0);
    }

    #[test]
    fn used_or_created_skill_is_a_milestone() {
        let skills = vec![
            SkillNode {
                name: "sealed".into(),
                category: "x".into(),
                created_by: Some("agent".into()),
                use_count: 0,
                related: vec![],
            },
            SkillNode {
                name: "used".into(),
                category: "x".into(),
                created_by: None,
                use_count: 3,
                related: vec![],
            },
        ];
        let graph = LearningGraph::build(skills, vec![]);
        assert_eq!(graph.learned_skill_count(), 2);
    }

    #[test]
    fn memory_links_to_overlapping_skill() {
        let skills = vec![SkillNode {
            name: "arxiv-search".into(),
            category: "research".into(),
            created_by: Some("agent".into()),
            use_count: 1,
            related: vec![],
        }];
        let memory = vec![MemoryCard::new(
            "memory",
            "arxiv search is the fastest way to find papers",
        )];
        let graph = LearningGraph::build(skills, memory);
        assert!(graph
            .edges
            .iter()
            .any(|e| e.source.starts_with("memory:") && e.target == "arxiv-search"));
    }

    #[test]
    fn fingerprint_is_stable() {
        assert_eq!(fingerprint("hello world"), fingerprint("  hello world  "));
    }
}
