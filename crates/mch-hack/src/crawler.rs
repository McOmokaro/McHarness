//! crawler.rs — Hackathon discovery across platforms.
//!
//! Provides `HackathonCrawler` for multi-platform discovery and `PlatformAdapter`
//! trait for extensible platform integrations.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CrawlerError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("platform adapter error: {0}")]
    Adapter(String),
    #[error("rate limited: {0}")]
    RateLimited(String),
}

pub type Result<T> = std::result::Result<T, CrawlerError>;

/// A discovered hackathon with metadata for triage.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Hackathon {
    pub id: Uuid,
    pub platform: Platform,
    pub title: String,
    pub description: String,
    pub url: String,
    pub starts_at: chrono::DateTime<chrono::Utc>,
    pub ends_at: chrono::DateTime<chrono::Utc>,
    pub prize_pool_usd: Option<u64>,
    pub tracks: Vec<String>,
    pub tags: Vec<String>,
    pub submission_format: SubmissionFormat,
    pub judging_criteria: Vec<JudgingCriterion>,
    pub raw_data: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Devpost,
    Gitcoin,
    EthGlobal,
    HackerEarth,
    Topcoder,
    Custom(String),
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Devpost => write!(f, "devpost"),
            Platform::Gitcoin => write!(f, "gitcoin"),
            Platform::EthGlobal => write!(f, "ethglobal"),
            Platform::HackerEarth => write!(f, "hackerearth"),
            Platform::Topcoder => write!(f, "topcoder"),
            Platform::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl std::str::FromStr for Platform {
    type Err = CrawlerError;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.to_lowercase().as_str() {
            "devpost" => Platform::Devpost,
            "gitcoin" => Platform::Gitcoin,
            "ethglobal" => Platform::EthGlobal,
            "hackerearth" => Platform::HackerEarth,
            "topcoder" => Platform::Topcoder,
            other => Platform::Custom(other.to_string()),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionFormat {
    GitRepo { url: String },
    ZipUpload { max_mb: u32 },
    DockerImage { registry: String },
    LiveDemo { url: String },
    Custom { schema: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JudgingCriterion {
    pub name: String,
    pub weight: f32,
    pub description: String,
    pub max_score: u32,
}

/// Trait for platform-specific discovery logic.
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    fn platform(&self) -> Platform;
    async fn fetch_hackathons(&self) -> Result<Vec<Hackathon>>;
    async fn fetch_details(&self, hackathon_id: &str) -> Result<Hackathon>;
    fn rate_limit(&self) -> RateLimit;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests_per_minute: u32,
    pub requests_per_hour: u32,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            requests_per_minute: 30,
            requests_per_hour: 500,
        }
    }
}

/// Multi-platform hackathon crawler.
pub struct HackathonCrawler {
    client: reqwest::Client,
    adapters: HashMap<Platform, Box<dyn PlatformAdapter>>,
}

impl HackathonCrawler {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("mch-hack/0.1")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("reqwest client");

        let mut adapters: HashMap<Platform, Box<dyn PlatformAdapter>> = HashMap::new();
        adapters.insert(Platform::Devpost, Box::new(DevpostAdapter::new(client.clone())));
        adapters.insert(Platform::Gitcoin, Box::new(GitcoinAdapter::new(client.clone())));
        adapters.insert(Platform::EthGlobal, Box::new(EthGlobalAdapter::new(client.clone())));
        adapters.insert(Platform::HackerEarth, Box::new(HackerEarthAdapter::new(client.clone())));
        adapters.insert(Platform::Topcoder, Box::new(TopcoderAdapter::new(client.clone())));

        Self { client, adapters }
    }

    pub fn register_adapter(&mut self, adapter: Box<dyn PlatformAdapter>) {
        self.adapters.insert(adapter.platform(), adapter);
    }

    /// Discover all hackathons across registered platforms.
    pub async fn discover(&self) -> Result<Vec<Hackathon>> {
        let mut all = Vec::new();
        for (platform, adapter) in &self.adapters {
            tracing::info!("discovering hackathons on {}", platform);
            match adapter.fetch_hackathons().await {
                Ok(mut hackathons) => {
                    tracing::info!("found {} hackathons on {}", hackathons.len(), platform);
                    all.append(&mut hackathons);
                }
                Err(e) => {
                    tracing::warn!("failed to fetch from {}: {}", platform, e);
                }
            }
        }
        Ok(all)
    }

    /// Get detailed info for a specific hackathon.
    pub async fn get_details(&self, platform: Platform, id: &str) -> Result<Hackathon> {
        let adapter = self.adapters.get(&platform)
            .ok_or_else(|| CrawlerError::Adapter(format!("no adapter for {}", platform)))?;
        adapter.fetch_details(id).await
    }

    /// Filter hackathons by active status.
    pub fn filter_active(hackathons: &[Hackathon]) -> Vec<Hackathon> {
        let now = chrono::Utc::now();
        hackathons
            .iter()
            .filter(|h| h.starts_at <= now && h.ends_at >= now)
            .cloned()
            .collect()
    }

    /// Filter by track/tags.
    pub fn filter_by_track(hackathons: &[Hackathon], track: &str) -> Vec<Hackathon> {
        hackathons
            .iter()
            .filter(|h| h.tracks.iter().any(|t| t.eq_ignore_ascii_case(track)))
            .cloned()
            .collect()
    }
}

impl Default for HackathonCrawler {
    fn default() -> Self {
        Self::new()
    }
}

/// Devpost adapter implementation.
struct DevpostAdapter {
    client: reqwest::Client,
}

impl DevpostAdapter {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformAdapter for DevpostAdapter {
    fn platform(&self) -> Platform {
        Platform::Devpost
    }

    async fn fetch_hackathons(&self) -> Result<Vec<Hackathon>> {
        // Devpost API: https://api.devpost.com/hackathons
        let url = "https://api.devpost.com/hackathons";
        let resp = self.client.get(url).send().await?;
        let data: serde_json::Value = resp.json().await?;

        // Parse Devpost response format
        let mut hackathons = Vec::new();
        if let Some(items) = data["hackathons"].as_array() {
            for item in items {
                if let Some(h) = Self::parse_hackathon(item) {
                    hackathons.push(h);
                }
            }
        }
        Ok(hackathons)
    }

    async fn fetch_details(&self, hackathon_id: &str) -> Result<Hackathon> {
        let url = format!("https://api.devpost.com/hackathons/{}", hackathon_id);
        let resp = self.client.get(&url).send().await?;
        let data: serde_json::Value = resp.json().await?;
        Self::parse_hackathon(&data).ok_or_else(|| CrawlerError::Parse("failed to parse details".into()))
    }

    fn rate_limit(&self) -> RateLimit {
        RateLimit::default()
    }
}

impl DevpostAdapter {
    fn parse_hackathon(v: &serde_json::Value) -> Option<Hackathon> {
        Some(Hackathon {
            id: Uuid::new_v4(),
            platform: Platform::Devpost,
            title: v["title"].as_str()?.to_string(),
            description: v["description"].as_str().unwrap_or("").to_string(),
            url: v["url"].as_str()?.to_string(),
            starts_at: v["starts_at"].as_str()?.parse().ok()?,
            ends_at: v["ends_at"].as_str()?.parse().ok()?,
            prize_pool_usd: v["prize_amount"].as_u64(),
            tracks: v["themes"].as_array()?.iter().filter_map(|t| t.as_str().map(String::from)).collect(),
            tags: v["tags"].as_array()?.iter().filter_map(|t| t.as_str().map(String::from)).collect(),
            submission_format: SubmissionFormat::GitRepo { url: "".into() },
            judging_criteria: vec![],
            raw_data: serde_json::from_value(v.clone()).ok()?,
        })
    }
}

/// Gitcoin adapter.
struct GitcoinAdapter {
    client: reqwest::Client,
}

impl GitcoinAdapter {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformAdapter for GitcoinAdapter {
    fn platform(&self) -> Platform {
        Platform::Gitcoin
    }

    async fn fetch_hackathons(&self) -> Result<Vec<Hackathon>> {
        // Gitcoin uses GraphQL
        let query = r#"
            query Hackathons {
                hackathons(first: 50) {
                    edges {
                        node {
                            id
                            title
                            description
                            url
                            startDate
                            endDate
                            prizePool
                            tracks
                            tags
                        }
                    }
                }
            }
        "#;
        let resp = self.client
            .post("https://api.gitcoin.co/graphql")
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await?;
        let data: serde_json::Value = resp.json().await?;

        let mut hackathons = Vec::new();
        if let Some(edges) = data["data"]["hackathons"]["edges"].as_array() {
            for edge in edges {
                if let Some(h) = Self::parse_hackathon(&edge["node"]) {
                    hackathons.push(h);
                }
            }
        }
        Ok(hackathons)
    }

    async fn fetch_details(&self, hackathon_id: &str) -> Result<Hackathon> {
        // Similar GraphQL query for single hackathon
        self.fetch_hackathons().await?
            .into_iter()
            .find(|h| h.id.to_string() == hackathon_id)
            .ok_or_else(|| CrawlerError::Parse("not found".into()))
    }

    fn rate_limit(&self) -> RateLimit {
        RateLimit { requests_per_minute: 20, ..Default::default() }
    }
}

impl GitcoinAdapter {
    fn parse_hackathon(v: &serde_json::Value) -> Option<Hackathon> {
        Some(Hackathon {
            id: Uuid::parse_str(v["id"].as_str()?).ok()?,
            platform: Platform::Gitcoin,
            title: v["title"].as_str()?.to_string(),
            description: v["description"].as_str().unwrap_or("").to_string(),
            url: v["url"].as_str()?.to_string(),
            starts_at: v["startDate"].as_str()?.parse().ok()?,
            ends_at: v["endDate"].as_str()?.parse().ok()?,
            prize_pool_usd: v["prizePool"].as_u64(),
            tracks: v["tracks"].as_array()?.iter().filter_map(|t| t.as_str().map(String::from)).collect(),
            tags: v["tags"].as_array()?.iter().filter_map(|t| t.as_str().map(String::from)).collect(),
            submission_format: SubmissionFormat::GitRepo { url: "".into() },
            judging_criteria: vec![],
            raw_data: serde_json::from_value(v.clone()).ok()?,
        })
    }
}

/// ETHGlobal adapter.
struct EthGlobalAdapter {
    client: reqwest::Client,
}

impl EthGlobalAdapter {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformAdapter for EthGlobalAdapter {
    fn platform(&self) -> Platform {
        Platform::EthGlobal
    }

    async fn fetch_hackathons(&self) -> Result<Vec<Hackathon>> {
        let resp = self.client.get("https://ethglobal.com/api/events").send().await?;
        let data: serde_json::Value = resp.json().await?;
        let mut hackathons = Vec::new();
        if let Some(events) = data["events"].as_array() {
            for e in events {
                if let Some(h) = Self::parse_hackathon(e) {
                    hackathons.push(h);
                }
            }
        }
        Ok(hackathons)
    }

    async fn fetch_details(&self, hackathon_id: &str) -> Result<Hackathon> {
        self.fetch_hackathons().await?
            .into_iter()
            .find(|h| h.id.to_string() == hackathon_id)
            .ok_or_else(|| CrawlerError::Parse("not found".into()))
    }

    fn rate_limit(&self) -> RateLimit {
        RateLimit::default()
    }
}

impl EthGlobalAdapter {
    fn parse_hackathon(v: &serde_json::Value) -> Option<Hackathon> {
        Some(Hackathon {
            id: Uuid::parse_str(v["id"].as_str()?).ok()?,
            platform: Platform::EthGlobal,
            title: v["name"].as_str()?.to_string(),
            description: v["description"].as_str().unwrap_or("").to_string(),
            url: v["url"].as_str().unwrap_or("").to_string(),
            starts_at: v["startDate"].as_str()?.parse().ok()?,
            ends_at: v["endDate"].as_str()?.parse().ok()?,
            prize_pool_usd: v["prizePool"].as_u64(),
            tracks: v["tracks"].as_array()?.iter().filter_map(|t| t.as_str().map(String::from)).collect(),
            tags: v["tags"].as_array()?.iter().filter_map(|t| t.as_str().map(String::from)).collect(),
            submission_format: SubmissionFormat::GitRepo { url: "".into() },
            judging_criteria: vec![],
            raw_data: serde_json::from_value(v.clone()).ok()?,
        })
    }
}

/// HackerEarth adapter.
struct HackerEarthAdapter {
    client: reqwest::Client,
}

impl HackerEarthAdapter {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformAdapter for HackerEarthAdapter {
    fn platform(&self) -> Platform {
        Platform::HackerEarth
    }

    async fn fetch_hackathons(&self) -> Result<Vec<Hackathon>> {
        let resp = self.client.get("https://api.hackerearth.com/v3/hackathons/").send().await?;
        let data: serde_json::Value = resp.json().await?;
        let mut hackathons = Vec::new();
        if let Some(items) = data["data"].as_array() {
            for item in items {
                if let Some(h) = Self::parse_hackathon(item) {
                    hackathons.push(h);
                }
            }
        }
        Ok(hackathons)
    }

    async fn fetch_details(&self, hackathon_id: &str) -> Result<Hackathon> {
        self.fetch_hackathons().await?
            .into_iter()
            .find(|h| h.id.to_string() == hackathon_id)
            .ok_or_else(|| CrawlerError::Parse("not found".into()))
    }

    fn rate_limit(&self) -> RateLimit {
        RateLimit { requests_per_minute: 10, ..Default::default() }
    }
}

impl HackerEarthAdapter {
    fn parse_hackathon(v: &serde_json::Value) -> Option<Hackathon> {
        Some(Hackathon {
            id: Uuid::parse_str(v["id"].as_str()?).ok()?,
            platform: Platform::HackerEarth,
            title: v["title"].as_str()?.to_string(),
            description: v["description"].as_str().unwrap_or("").to_string(),
            url: v["url"].as_str().unwrap_or("").to_string(),
            starts_at: v["start_date"].as_str()?.parse().ok()?,
            ends_at: v["end_date"].as_str()?.parse().ok()?,
            prize_pool_usd: v["prize"].as_u64(),
            tracks: v["tracks"].as_array()?.iter().filter_map(|t| t.as_str().map(String::from)).collect(),
            tags: v["tags"].as_array()?.iter().filter_map(|t| t.as_str().map(String::from)).collect(),
            submission_format: SubmissionFormat::ZipUpload { max_mb: 100 },
            judging_criteria: vec![],
            raw_data: serde_json::from_value(v.clone()).ok()?,
        })
    }
}

/// Topcoder adapter.
struct TopcoderAdapter {
    client: reqwest::Client,
}

impl TopcoderAdapter {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformAdapter for TopcoderAdapter {
    fn platform(&self) -> Platform {
        Platform::Topcoder
    }

    async fn fetch_hackathons(&self) -> Result<Vec<Hackathon>> {
        // Topcoder uses SRM/Marathon match APIs
        let resp = self.client.get("https://api.topcoder.com/v5/challenges").send().await?;
        let data: serde_json::Value = resp.json().await?;
        let mut hackathons = Vec::new();
        if let Some(items) = data.as_array() {
            for item in items {
                if let Some(h) = Self::parse_hackathon(item) {
                    hackathons.push(h);
                }
            }
        }
        Ok(hackathons)
    }

    async fn fetch_details(&self, hackathon_id: &str) -> Result<Hackathon> {
        self.fetch_hackathons().await?
            .into_iter()
            .find(|h| h.id.to_string() == hackathon_id)
            .ok_or_else(|| CrawlerError::Parse("not found".into()))
    }

    fn rate_limit(&self) -> RateLimit {
        RateLimit { requests_per_minute: 15, ..Default::default() }
    }
}

impl TopcoderAdapter {
    fn parse_hackathon(v: &serde_json::Value) -> Option<Hackathon> {
        Some(Hackathon {
            id: Uuid::parse_str(v["challengeId"].as_str()?).ok()?,
            platform: Platform::Topcoder,
            title: v["challengeName"].as_str()?.to_string(),
            description: v["description"].as_str().unwrap_or("").to_string(),
            url: v["registrationUrl"].as_str().unwrap_or("").to_string(),
            starts_at: v["registrationStartDate"].as_str()?.parse().ok()?,
            ends_at: v["registrationEndDate"].as_str()?.parse().ok()?,
            prize_pool_usd: v["totalPrize"].as_u64(),
            tracks: vec!["algorithmic".into()],
            tags: v["technologies"].as_array()?.iter().filter_map(|t| t.as_str().map(String::from)).collect(),
            submission_format: SubmissionFormat::ZipUpload { max_mb: 50 },
            judging_criteria: vec![],
            raw_data: serde_json::from_value(v.clone()).ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_display() {
        assert_eq!(Platform::Devpost.to_string(), "devpost");
        assert_eq!(Platform::Gitcoin.to_string(), "gitcoin");
        assert_eq!(Platform::Custom("foo".into()).to_string(), "foo");
    }

    #[test]
    fn platform_from_str() {
        assert_eq!("devpost".parse::<Platform>().unwrap(), Platform::Devpost);
        assert_eq!("gitcoin".parse::<Platform>().unwrap(), Platform::Gitcoin);
        assert_eq!("custom".parse::<Platform>().unwrap(), Platform::Custom("custom".into()));
    }

    #[test]
    fn hackathon_crawler_constructs() {
        let _crawler = HackathonCrawler::new();
    }

    #[tokio::test]
    async fn filter_active() {
        let now = chrono::Utc::now();
        let h1 = Hackathon {
            id: Uuid::new_v4(),
            platform: Platform::Devpost,
            title: "Active".into(),
            description: "".into(),
            url: "".into(),
            starts_at: now - chrono::Duration::hours(1),
            ends_at: now + chrono::Duration::hours(1),
            prize_pool_usd: None,
            tracks: vec![],
            tags: vec![],
            submission_format: SubmissionFormat::GitRepo { url: "".into() },
            judging_criteria: vec![],
            raw_data: Default::default(),
        };
        let h2 = Hackathon {
            id: Uuid::new_v4(),
            platform: Platform::Devpost,
            title: "Future".into(),
            description: "".into(),
            url: "".into(),
            starts_at: now + chrono::Duration::days(1),
            ends_at: now + chrono::Duration::days(2),
            prize_pool_usd: None,
            tracks: vec![],
            tags: vec![],
            submission_format: SubmissionFormat::GitRepo { url: "".into() },
            judging_criteria: vec![],
            raw_data: Default::default(),
        };
        let filtered = HackathonCrawler::filter_active(&[h1.clone(), h2]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Active");
    }
}