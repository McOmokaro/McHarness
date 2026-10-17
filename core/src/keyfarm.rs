//! The Key Farm: all API keys act as one pool.
//!
//! Name the model — the farm finds every key that carries it, orders
//! them as fallbacks, watches rate limits, and reroutes before failure.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FarmKey {
    pub id: String,
    pub provider: String,
    pub models: Vec<String>,
    pub usage_count: u64,
    pub rate_limit_pct: f32,
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLeadboardEntry {
    pub domain: String,
    pub ranked_models: Vec<String>,
}

pub struct KeyFarm {
    keys: Vec<FarmKey>,
    leaderboards: HashMap<String, Vec<String>>,
}

impl KeyFarm {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            leaderboards: HashMap::new(),
        }
    }

    pub fn add_key(&mut self, key: FarmKey) {
        self.keys.push(key);
    }

    pub fn seed_leaderboard(&mut self, domain: &str, ranked_models: Vec<String>) {
        self.leaderboards.insert(domain.to_string(), ranked_models);
    }

    /// Resolve (model, key) for a task. Preference first; leaderboard fallback.
    pub fn resolve(&mut self, preference: &str, domain: Option<&str>) -> Result<(String, String)> {
        if preference == "auto" {
            let domain = domain.unwrap_or("general");
            if let Some(ranked) = self.leaderboards.get(domain) {
                for model in ranked {
                    if let Some(key) = self.find_key_for(model)? {
                        return Ok((model.clone(), key));
                    }
                }
            }
        }

        if let Some(key) = self.find_key_for(preference)? {
            return Ok((preference.to_string(), key));
        }

        // Fallback: next best proven model for the domain.
        if let Some(domain) = domain {
            if let Some(ranked) = self.leaderboards.get(domain) {
                for model in ranked {
                    if let Some(key) = self.find_key_for(model)? {
                        return Ok((model.clone(), key));
                    }
                }
            }
        }

        anyhow::bail!("no available key/model pair for preference '{preference}'")
    }

    fn find_key_for(&mut self, model: &str) -> Result<Option<String>> {
        // Keys carrying the model, sorted by lowest rate usage, unpaused.
        let mut candidates: Vec<&FarmKey> = self
            .keys
            .iter()
            .filter(|k| !k.paused && k.models.iter().any(|m| m == model))
            .collect();
        candidates.sort_by(|a, b| a.rate_limit_pct.partial_cmp(&b.rate_limit_pct).unwrap());

        if let Some(key) = candidates.first() {
            let id = key.id.clone();
            let idx = self.keys.iter().position(|k| k.id == id).unwrap();
            self.keys[idx].usage_count += 1;
            // Telemetry: approach the pause threshold before failure.
            self.keys[idx].rate_limit_pct += 1.0;
            if self.keys[idx].rate_limit_pct >= 90.0 {
                self.keys[idx].paused = true;
            }
            return Ok(Some(id));
        }
        Ok(None)
    }

    /// TUI telemetry feed: key health for the live panel.
    pub fn telemetry(&self) -> Vec<FarmKey> {
        self.keys.clone()
    }
}
