//! Fission: all plugins as one — same pattern as Fusion, for plugins.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub domain: String,
    pub entry_manifest: String,
}

pub struct Fission {
    plugins_by_domain: HashMap<String, Vec<Plugin>>,
}

impl Fission {
    pub fn new() -> Self {
        Self {
            plugins_by_domain: HashMap::new(),
        }
    }

    /// Split-in a plugin; arms by domain like Fusion.
    pub fn split_in(&mut self, plugin: Plugin) {
        self.plugins_by_domain
            .entry(plugin.domain.clone())
            .or_default()
            .push(plugin);
    }

    pub fn arm(&self, domain: &str) -> Vec<&Plugin> {
        self.plugins_by_domain.get(domain).map(|v| v.iter().collect()).unwrap_or_default()
    }

    /// One-command plugin-bundle merge (a Fission update).
    pub fn split_in_batch(&mut self, plugins: Vec<Plugin>) {
        for p in plugins {
            self.split_in(p);
        }
    }

    pub fn load_manifest(_manifest: &str) -> Result<Vec<Plugin>> {
        // v0.0.1 stub: the Fission update loader lands with the Ingestor.
        Ok(Vec::new())
    }
}
