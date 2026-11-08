//! Working Memory (Episodic) — token-budget-managed session context

use crate::types::{SessionId, TokenBudget};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tiktoken_rs::{cl100k_base, CoreBPE};
use tracing::{debug, info};

/// A single conversation turn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub token_count: usize,
    pub turn_index: usize,
}

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

/// Compressed history structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedHistory {
    /// Summary anchors (past context, distilled)
    pub summaries: Vec<SummaryAnchor>,
    /// Raw recent turns (verbatim)
    pub recent_turns: Vec<Turn>,
    /// Total tokens in compressed history
    pub total_tokens: usize,
}

/// Summary anchor for compressed context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryAnchor {
    pub turn_range: (usize, usize),
    pub summary: String,
    pub token_count: usize,
    pub key_topics: Vec<String>,
}

/// Working memory configuration
#[derive(Debug, Clone)]
pub struct WorkingConfig {
    pub max_tokens: usize,
    pub warn_threshold: f32,
    pub block_threshold: f32,
    pub max_recent_turns: usize,
    pub compression_model: String,
}

impl Default for WorkingConfig {
    fn default() -> Self {
        Self {
            max_tokens: 8192,
            warn_threshold: 0.7,
            block_threshold: 0.9,
            max_recent_turns: 10,
            compression_model: "gpt-4o-mini".to_string(),
        }
    }
}

/// Working Memory manager
pub struct WorkingMemory {
    config: WorkingConfig,
    budget: TokenBudget,
    turns: VecDeque<Turn>,
    turn_counter: usize,
    tokenizer: CoreBPE,
    compressed: Option<CompressedHistory>,
}

impl WorkingMemory {
    pub fn new(config: WorkingConfig) -> Result<Self> {
        let tokenizer = cl100k_base()?;
        Ok(Self {
            budget: TokenBudget::new(config.max_tokens),
            config,
            turns: VecDeque::new(),
            turn_counter: 0,
            tokenizer,
            compressed: None,
        })
    }

    /// Append a user turn
    pub fn append_user(&mut self, content: String) -> Result<()> {
        self.append_turn(Role::User, content)
    }

    /// Append an assistant turn
    pub fn append_assistant(&mut self, content: String) -> Result<()> {
        self.append_turn(Role::Assistant, content)
    }

    fn append_turn(&mut self, role: Role, content: String) -> Result<()> {
        let tokens = self.count_tokens(&content)?;
        let turn = Turn {
            role,
            content,
            timestamp: Utc::now(),
            token_count: tokens,
            turn_index: self.turn_counter,
        };
        self.turn_counter += 1;
        self.budget.current_tokens += tokens;
        self.turns.push_back(turn);
        debug!("Appended {} turn, budget: {}/{}", role as u8, self.budget.current_tokens, self.budget.max_tokens);
        
        // Check thresholds
        if self.budget.is_warn() {
            info!("Token budget warn threshold reached ({}%)", (self.budget.usage_ratio() * 100.0) as u32);
        }
        if self.budget.is_blocked() {
            info!("Token budget block threshold reached, compressing...");
            self.compress()?;
        }
        Ok(())
    }

    /// Get current working memory state for downstream stages
    pub fn get_state(&self) -> (VecDeque<Turn>, Option<CompressedHistory>) {
        (self.turns.clone(), self.compressed.clone())
    }

    /// Get token budget status
    pub fn budget(&self) -> &TokenBudget {
        &self.budget
    }

    /// Compress older turns into summaries
    pub fn compress(&mut self) -> Result<()> {
        if self.turns.len() <= self.config.max_recent_turns {
            return Ok(());
        }

        // Split into recent (keep raw) and older (compress)
        let split_idx = self.turns.len() - self.config.max_recent_turns;
        let mut older_turns: Vec<Turn> = self.turns.drain(..split_idx).collect();
        let recent_turns: Vec<Turn> = self.turns.drain(..).collect();

        // Create summary anchors from older turns
        let summaries = self.create_summaries(&older_turns)?;

        // Calculate tokens
        let summary_tokens: usize = summaries.iter().map(|s| s.token_count).sum();
        let recent_tokens: usize = recent_turns.iter().map(|t| t.token_count).sum();

        self.compressed = Some(CompressedHistory {
            summaries,
            recent_turns: recent_turns.clone(),
            total_tokens: summary_tokens + recent_tokens,
        });

        // Restore recent turns to deque
        self.turns = recent_turns.into_iter().collect();
        self.budget.current_tokens = self.compressed.as_ref().unwrap().total_tokens;

        info!("Compressed {} older turns into {} summary anchors, budget now: {}/{}",
              older_turns.len(), self.compressed.as_ref().unwrap().summaries.len(),
              self.budget.current_tokens, self.budget.max_tokens);
        Ok(())
    }

    fn create_summaries(&self, turns: &[Turn]) -> Result<Vec<SummaryAnchor>> {
        // In practice, this would call an LLM to generate summaries
        // For now, create simple heuristic summaries
        let mut summaries = Vec::new();
        let chunk_size = 5;
        
        for chunk in turns.chunks(chunk_size) {
            let start = chunk.first().map(|t| t.turn_index).unwrap_or(0);
            let end = chunk.last().map(|t| t.turn_index).unwrap_or(0);
            let content = chunk.iter()
                .map(|t| format!("{}: {}", t.role as u8, t.content))
                .collect::<Vec<_>>()
                .join("\n");
            
            // Simple extractive summary (first 200 chars)
            let summary = if content.len() > 200 {
                format!("{}...", &content[..200])
            } else {
                content.clone()
            };
            
            let token_count = self.count_tokens(&summary)?;
            let key_topics = self.extract_topics(&content);
            
            summaries.push(SummaryAnchor {
                turn_range: (start, end),
                summary,
                token_count,
                key_topics,
            });
        }
        Ok(summaries)
    }

    fn extract_topics(&self, content: &str) -> Vec<String> {
        // Simple keyword extraction (in practice, use LLM)
        let keywords = ["database", "backup", "api", "config", "error", "deploy", "test", "security"];
        keywords.iter()
            .filter(|k| content.to_lowercase().contains(*k))
            .map(|s| s.to_string())
            .collect()
    }

    /// Count tokens using tiktoken
    pub fn count_tokens(&self, text: &str) -> Result<usize> {
        let tokens = self.tokenizer.encode_with_special_tokens(text);
        Ok(tokens.len())
    }

    /// Reset for new session
    pub fn reset(&mut self) {
        self.turns.clear();
        self.turn_counter = 0;
        self.budget.current_tokens = 0;
        self.compressed = None;
    }

    /// Get full conversation history (compressed + recent)
    pub fn full_history(&self) -> String {
        let mut parts = Vec::new();
        
        if let Some(compressed) = &self.compressed {
            for anchor in &compressed.summaries {
                parts.push(format!("[Summary turns {}-{}]: {}", anchor.turn_range.0, anchor.turn_range.1, anchor.summary));
            }
        }
        
        for turn in &self.turns {
            parts.push(format!("{}: {}", turn.role as u8, turn.content));
        }
        
        parts.join("\n")
    }
}