//! Hackathon crawler and platform adapters.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A discovered hackathon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hackathon {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub url: String,
    pub prize_pool: Option<String>,
    pub themes: Vec<String>,
    pub deadline: Option<String>,
}

/// Known hackathon platforms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Devpost,
    MlH,
    Devfolio,
    EthGlobal,
    Custom(String),
}

/// An adapter that can discover (and submit to) a platform.
pub trait PlatformAdapter: Send + Sync {
    fn platform_name(&self) -> &str;
    fn discover(&self) -> Vec<Hackathon>;
}

/// Crawler that fans out across registered platform adapters.
pub struct HackathonCrawler {
    adapters: HashMap<String, Box<dyn PlatformAdapter>>,
}

impl HackathonCrawler {
    pub fn new() -> Self {
        Self { adapters: HashMap::new() }
    }

    pub fn register_adapter(&mut self, adapter: Box<dyn PlatformAdapter>) {
        self.adapters.insert(adapter.platform_name().to_string(), adapter);
    }

    /// Discover hackathons across all adapters.
    pub async fn discover(&self) -> anyhow::Result<Vec<Hackathon>> {
        let mut all = Vec::new();
        for adapter in self.adapters.values() {
            all.extend(adapter.discover());
        }
        Ok(all)
    }
}

impl Default for HackathonCrawler {
    fn default() -> Self {
        Self::new()
    }
}

/// An in-memory adapter, useful for tests and deterministic runs.
pub struct StaticAdapter {
    name: String,
    items: Vec<Hackathon>,
}

impl StaticAdapter {
    pub fn new(name: impl Into<String>, items: Vec<Hackathon>) -> Self {
        Self { name: name.into(), items }
    }
}

impl PlatformAdapter for StaticAdapter {
    fn platform_name(&self) -> &str {
        &self.name
    }
    fn discover(&self) -> Vec<Hackathon> {
        self.items.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn discovers_across_adapters() {
        let mut crawler = HackathonCrawler::new();
        crawler.register_adapter(Box::new(StaticAdapter::new(
            "devpost",
            vec![Hackathon {
                id: "h1".into(),
                name: "AI Hack".into(),
                platform: "devpost".into(),
                url: "https://devpost.com/x".into(),
                prize_pool: Some("$50k".into()),
                themes: vec!["ai".into()],
                deadline: None,
            }],
        )));
        assert_eq!(crawler.discover().await.unwrap().len(), 1);
    }
}
