//! Report compilation and data aggregation for Morning Report.
//!
//! Collects data from multiple sources (communications, economic engine,
//! hackathon results) and compiles into a unified ReportData structure
//! ready for newspaper generation and TTS synthesis.

use crate::newspaper::{CommunicationItem, EconomicSummary, HackathonSummary, Priority};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Compiled report data ready for newspaper/TTs generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportData {
    pub date: chrono::DateTime<chrono::Utc>,
    pub headlines: Vec<String>,
    pub communications: Vec<CommunicationItem>,
    pub economic: EconomicSummary,
    pub hackathon: HackathonSummary,
    pub metrics: HashMap<String, serde_json::Value>,
    pub footnotes: Vec<String>,
    pub custom_sections: HashMap<String, String>,
}

/// Compiles the morning report from aggregated data sources.
#[derive(Debug)]
pub struct ReportCompiler {
    aggregator: Arc<DataAggregator>,
    template: ReportTemplate,
}

impl ReportCompiler {
    /// Create a new compiler with the given aggregator.
    pub fn new(aggregator: Arc<DataAggregator>) -> Self {
        Self {
            aggregator,
            template: ReportTemplate::default(),
        }
    }

    /// Set custom report template.
    pub fn with_template(mut self, template: ReportTemplate) -> Self {
        self.template = template;
        self
    }

    /// Compile the full morning report for the given date.
    pub async fn compile(&self, date: chrono::DateTime<chrono::Utc>) -> Result<ReportData> {
        let mut data = ReportData {
            date,
            headlines: Vec::new(),
            communications: Vec::new(),
            economic: EconomicSummary::default(),
            hackathon: HackathonSummary::default(),
            metrics: HashMap::new(),
            footnotes: Vec::new(),
            custom_sections: HashMap::new(),
        };

        // Aggregate from all sources in parallel
        let (headlines, comms, economic, hackathon, metrics) = tokio::join!(
            self.aggregator.fetch_headlines(date),
            self.aggregator.fetch_communications(date),
            self.aggregator.fetch_economic_summary(date),
            self.aggregator.fetch_hackathon_results(date),
            self.aggregator.fetch_metrics(date),
        );

        data.headlines = headlines?;
        data.communications = comms?;
        data.economic = economic?;
        data.hackathon = hackathon?;
        data.metrics = metrics?;

        // Apply template transformations
        self.apply_template(&mut data);

        Ok(data)
    }

    /// Compile and generate newspaper Markdown.
    pub async fn compile_newspaper(&self, date: chrono::DateTime<chrono::Utc>) -> Result<String> {
        let data = self.compile(date).await?;
        let generator = self.template.newspaper_generator();
        generator.generate(&data)
    }

    /// Compile and generate TTS audio.
    pub async fn compile_audio(&self, date: chrono::DateTime<chrono::Utc>) -> Result<crate::tts::AudioOutput> {
        let data = self.compile(date).await?;
        let newspaper = self.template.newspaper_generator().generate(&data)?;
        self.template.tts_generator().generate(&newspaper, &date).await
    }

    fn apply_template(&self, data: &mut ReportData) {
        // Add template footnotes
        data.footnotes.extend(self.template.footnotes.clone());

        // Add custom sections from template
        for (name, content) in &self.template.custom_sections {
            data.custom_sections.insert(name.clone(), content.clone());
        }
    }
}

/// Aggregates data from multiple sources for report compilation.
#[derive(Debug)]
pub struct DataAggregator {
    /// Communications source (Slack, email, etc.)
    comms_source: Arc<dyn CommunicationsSource + Send + Sync>,
    /// Economic engine source
    econ_source: Arc<dyn EconomicSource + Send + Sync>,
    /// Hackathon results source
    hackathon_source: Arc<dyn HackathonSource + Send + Sync>,
    /// Metrics source
    metrics_source: Arc<dyn MetricsSource + Send + Sync>,
    /// Headlines source
    headlines_source: Arc<dyn HeadlinesSource + Send + Sync>,
    /// Cache for fetched data
    cache: Arc<RwLock<HashMap<String, CachedData>>>,
}

impl DataAggregator {
    /// Create a new aggregator with all sources.
    pub fn new(
        comms_source: Arc<dyn CommunicationsSource + Send + Sync>,
        econ_source: Arc<dyn EconomicSource + Send + Sync>,
        hackathon_source: Arc<dyn HackathonSource + Send + Sync>,
        metrics_source: Arc<dyn MetricsSource + Send + Sync>,
        headlines_source: Arc<dyn HeadlinesSource + Send + Sync>,
    ) -> Self {
        Self {
            comms_source,
            econ_source,
            hackathon_source,
            metrics_source,
            headlines_source,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a builder for configuring sources.
    pub fn builder() -> DataAggregatorBuilder {
        DataAggregatorBuilder::default()
    }

    /// Fetch headlines for the given date.
    pub async fn fetch_headlines(&self, date: chrono::DateTime<chrono::Utc>) -> Result<Vec<String>> {
        let key = format!("headlines_{}", date.format("%Y%m%d"));
        if let Some(cached) = self.get_cached(&key).await {
            return Ok(cached.headlines);
        }
        let headlines = self.headlines_source.fetch(date).await?;
        self.cache.insert(key, CachedData { headlines: headlines.clone(), ..Default::default() }).await;
        Ok(headlines)
    }

    /// Fetch communications for the given date.
    pub async fn fetch_communications(&self, date: chrono::DateTime<chrono::Utc>) -> Result<Vec<CommunicationItem>> {
        let key = format!("comms_{}", date.format("%Y%m%d"));
        if let Some(cached) = self.get_cached(&key).await {
            return Ok(cached.communications);
        }
        let comms = self.comms_source.fetch(date).await?;
        self.cache.insert(key, CachedData { communications: comms.clone(), ..Default::default() }).await;
        Ok(comms)
    }

    /// Fetch economic summary for the given date.
    pub async fn fetch_economic_summary(&self, date: chrono::DateTime<chrono::Utc>) -> Result<EconomicSummary> {
        let key = format!("econ_{}", date.format("%Y%m%d"));
        if let Some(cached) = self.get_cached(&key).await {
            return Ok(cached.economic);
        }
        let econ = self.econ_source.fetch(date).await?;
        self.cache.insert(key, CachedData { economic: econ.clone(), ..Default::default() }).await;
        Ok(econ)
    }

    /// Fetch hackathon results for the given date.
    pub async fn fetch_hackathon_results(&self, date: chrono::DateTime<chrono::Utc>) -> Result<HackathonSummary> {
        let key = format!("hack_{}", date.format("%Y%m%d"));
        if let Some(cached) = self.get_cached(&key).await {
            return Ok(cached.hackathon);
        }
        let hack = self.hackathon_source.fetch(date).await?;
        self.cache.insert(key, CachedData { hackathon: hack.clone(), ..Default::default() }).await;
        Ok(hack)
    }

    /// Fetch metrics for the given date.
    pub async fn fetch_metrics(&self, date: chrono::DateTime<chrono::Utc>) -> Result<HashMap<String, serde_json::Value>> {
        let key = format!("metrics_{}", date.format("%Y%m%d"));
        if let Some(cached) = self.get_cached(&key).await {
            return Ok(cached.metrics);
        }
        let metrics = self.metrics_source.fetch(date).await?;
        self.cache.insert(key, CachedData { metrics: metrics.clone(), ..Default::default() }).await;
        Ok(metrics)
    }

    async fn get_cached(&self, key: &str) -> Option<CachedData> {
        self.cache.read().await.get(key).cloned()
    }

    async fn cache_insert(&self, key: String, data: CachedData) {
        self.cache.write().await.insert(key, data);
    }

    /// Clear the cache.
    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
    }
}

#[derive(Debug, Default, Clone)]
struct CachedData {
    headlines: Vec<String>,
    communications: Vec<CommunicationItem>,
    economic: EconomicSummary,
    hackathon: HackathonSummary,
    metrics: HashMap<String, serde_json::Value>,
}

/// Builder for DataAggregator with optional sources.
#[derive(Default)]
pub struct DataAggregatorBuilder {
    comms_source: Option<Arc<dyn CommunicationsSource + Send + Sync>>,
    econ_source: Option<Arc<dyn EconomicSource + Send + Sync>>,
    hackathon_source: Option<Arc<dyn HackathonSource + Send + Sync>>,
    metrics_source: Option<Arc<dyn MetricsSource + Send + Sync>>,
    headlines_source: Option<Arc<dyn HeadlinesSource + Send + Sync>>,
}

impl DataAggregatorBuilder {
    pub fn comms_source(mut self, source: Arc<dyn CommunicationsSource + Send + Sync>) -> Self {
        self.comms_source = Some(source);
        self
    }

    pub fn econ_source(mut self, source: Arc<dyn EconomicSource + Send + Sync>) -> Self {
        self.econ_source = Some(source);
        self
    }

    pub fn hackathon_source(mut self, source: Arc<dyn HackathonSource + Send + Sync>) -> Self {
        self.hackathon_source = Some(source);
        self
    }

    pub fn metrics_source(mut self, source: Arc<dyn MetricsSource + Send + Sync>) -> Self {
        self.metrics_source = Some(source);
        self
    }

    pub fn headlines_source(mut self, source: Arc<dyn HeadlinesSource + Send + Sync>) -> Self {
        self.headlines_source = Some(source);
        self
    }

    pub fn build(self) -> Result<DataAggregator> {
        Ok(DataAggregator::new(
            self.comms_source.ok_or_else(|| anyhow::anyhow!("comms source required"))?,
            self.econ_source.ok_or_else(|| anyhow::anyhow!("econ source required"))?,
            self.hackathon_source.ok_or_else(|| anyhow::anyhow!("hackathon source required"))?,
            self.metrics_source.ok_or_else(|| anyhow::anyhow!("metrics source required"))?,
            self.headlines_source.ok_or_else(|| anyhow::anyhow!("headlines source required"))?,
        ))
    }
}

/// Report template for customizing output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportTemplate {
    pub footnotes: Vec<String>,
    pub custom_sections: HashMap<String, String>,
    pub newspaper_columns: usize,
    pub newspaper_ascii_header: bool,
    pub tts_rate: f32,
    pub tts_voice: Option<String>,
}

impl Default for ReportTemplate {
    fn default() -> Self {
        Self {
            footnotes: vec![
                "All times UTC".to_string(),
                "Data sourced from internal systems".to_string(),
            ],
            custom_sections: HashMap::new(),
            newspaper_columns: 1,
            newspaper_ascii_header: true,
            tts_rate: 1.0,
            tts_voice: None,
        }
    }
}

impl ReportTemplate {
    pub fn newspaper_generator(&self) -> crate::newspaper::NewspaperGenerator {
        crate::newspaper::NewspaperGenerator::new()
            .with_columns(self.newspaper_columns)
            .with_ascii_header(self.newspaper_ascii_header)
    }

    pub fn tts_generator(&self) -> crate::tts::TTSGenerator {
        let mut gen = crate::tts::TTSGenerator::new().with_rate(self.tts_rate);
        if let Some(voice) = &self.tts_voice {
            gen = gen.with_voice_selector(crate::tts::VoiceSelector::new().with_voice(voice.clone()));
        }
        gen
    }
}

/// Trait for fetching communications data.
#[async_trait::async_trait]
pub trait CommunicationsSource {
    async fn fetch(&self, date: chrono::DateTime<chrono::Utc>) -> Result<Vec<CommunicationItem>>;
}

/// Trait for fetching economic data.
#[async_trait::async_trait]
pub trait EconomicSource {
    async fn fetch(&self, date: chrono::DateTime<chrono::Utc>) -> Result<EconomicSummary>;
}

/// Trait for fetching hackathon results.
#[async_trait::async_trait]
pub trait HackathonSource {
    async fn fetch(&self, date: chrono::DateTime<chrono::Utc>) -> Result<HackathonSummary>;
}

/// Trait for fetching metrics.
#[async_trait::async_trait]
pub trait MetricsSource {
    async fn fetch(&self, date: chrono::DateTime<chrono::Utc>) -> Result<HashMap<String, serde_json::Value>>;
}

/// Trait for fetching headlines.
#[async_trait::async_trait]
pub trait HeadlinesSource {
    async fn fetch(&self, date: chrono::DateTime<chrono::Utc>) -> Result<Vec<String>>;
}

/// Mock implementations for testing.
#[cfg(test)]
mod mocks {
    use super::*;
    use chrono::{TimeZone, Utc};

    pub struct MockCommsSource;
    #[async_trait::async_trait]
    impl CommunicationsSource for MockCommsSource {
        async fn fetch(&self, _date: chrono::DateTime<chrono::Utc>) -> Result<Vec<CommunicationItem>> {
            Ok(vec![CommunicationItem {
                timestamp: Utc.with_ymd_and_hms(2026, 1, 15, 5, 30, 0).unwrap(),
                channel: "test".to_string(),
                summary: "Mock communication".to_string(),
                priority: Priority::Normal,
            }])
        }
    }

    pub struct MockEconSource;
    #[async_trait::async_trait]
    impl EconomicSource for MockEconSource {
        async fn fetch(&self, _date: chrono::DateTime<chrono::Utc>) -> Result<EconomicSummary> {
            Ok(EconomicSummary {
                portfolio_value: 1_000_000.0,
                daily_pnl: 5_000.0,
                daily_pnl_pct: 0.5,
                top_mover: "MOCK".to_string(),
                top_mover_pct: 2.0,
                active_positions: 5,
                notes: vec![],
            })
        }
    }

    pub struct MockHackathonSource;
    #[async_trait::async_trait]
    impl HackathonSource for MockHackathonSource {
        async fn fetch(&self, _date: chrono::DateTime<chrono::Utc>) -> Result<HackathonSummary> {
            Ok(HackathonSummary {
                event_name: "Mock Hackathon".to_string(),
                rank: 1,
                total_teams: 10,
                score: 100.0,
                category: "Test".to_string(),
                highlights: vec![],
            })
        }
    }

    pub struct MockMetricsSource;
    #[async_trait::async_trait]
    impl MetricsSource for MockMetricsSource {
        async fn fetch(&self, _date: chrono::DateTime<chrono::Utc>) -> Result<HashMap<String, serde_json::Value>> {
            let mut m = HashMap::new();
            m.insert("uptime".to_string(), serde_json::json!(99.9));
            Ok(m)
        }
    }

    pub struct MockHeadlinesSource;
    #[async_trait::async_trait]
    impl HeadlinesSource for MockHeadlinesSource {
        async fn fetch(&self, _date: chrono::DateTime<chrono::Utc>) -> Result<Vec<String>> {
            Ok(vec!["Mock Headline 1".to_string(), "Mock Headline 2".to_string()])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mocks::*;
    use chrono::{TimeZone, Utc};

    #[tokio::test]
    async fn data_aggregator_fetches_all_sources() {
        let aggregator = DataAggregator::builder()
            .comms_source(Arc::new(MockCommsSource))
            .econ_source(Arc::new(MockEconSource))
            .hackathon_source(Arc::new(MockHackathonSource))
            .metrics_source(Arc::new(MockMetricsSource))
            .headlines_source(Arc::new(MockHeadlinesSource))
            .build()
            .unwrap();

        let date = Utc.with_ymd_and_hms(2026, 1, 15, 6, 0, 0).unwrap();

        let headlines = aggregator.fetch_headlines(date).await.unwrap();
        assert_eq!(headlines.len(), 2);

        let comms = aggregator.fetch_communications(date).await.unwrap();
        assert_eq!(comms.len(), 1);

        let econ = aggregator.fetch_economic_summary(date).await.unwrap();
        assert_eq!(econ.portfolio_value, 1_000_000.0);

        let hack = aggregator.fetch_hackathon_results(date).await.unwrap();
        assert_eq!(hack.rank, 1);

        let metrics = aggregator.fetch_metrics(date).await.unwrap();
        assert!(metrics.contains_key("uptime"));
    }

    #[tokio::test]
    async fn report_compiler_compiles_full_report() {
        let aggregator = Arc::new(
            DataAggregator::builder()
                .comms_source(Arc::new(MockCommsSource))
                .econ_source(Arc::new(MockEconSource))
                .hackathon_source(Arc::new(MockHackathonSource))
                .metrics_source(Arc::new(MockMetricsSource))
                .headlines_source(Arc::new(MockHeadlinesSource))
                .build()
                .unwrap(),
        );

        let compiler = ReportCompiler::new(aggregator);
        let date = Utc.with_ymd_and_hms(2026, 1, 15, 6, 0, 0).unwrap();

        let data = compiler.compile(date).await.unwrap();

        assert_eq!(data.headlines.len(), 2);
        assert_eq!(data.communications.len(), 1);
        assert_eq!(data.economic.portfolio_value, 1_000_000.0);
        assert_eq!(data.hackathon.rank, 1);
        assert!(data.metrics.contains_key("uptime"));
    }

    #[tokio::test]
    async fn report_compiler_generates_newspaper() {
        let aggregator = Arc::new(
            DataAggregator::builder()
                .comms_source(Arc::new(MockCommsSource))
                .econ_source(Arc::new(MockEconSource))
                .hackathon_source(Arc::new(MockHackathonSource))
                .metrics_source(Arc::new(MockMetricsSource))
                .headlines_source(Arc::new(MockHeadlinesSource))
                .build()
                .unwrap(),
        );

        let compiler = ReportCompiler::new(aggregator);
        let date = Utc.with_ymd_and_hms(2026, 1, 15, 6, 0, 0).unwrap();

        let newspaper = compiler.compile_newspaper(date).await.unwrap();

        assert!(newspaper.contains("MORNING REPORT"));
        assert!(newspaper.contains("HEADLINES"));
        assert!(newspaper.contains("Mock Headline"));
    }

    #[tokio::test]
    async fn report_template_customization() {
        let template = ReportTemplate {
            footnotes: vec!["Custom footnote".to_string()],
            newspaper_columns: 2,
            newspaper_ascii_header: false,
            ..Default::default()
        };

        let gen = template.newspaper_generator();
        assert_eq!(gen.columns, 2);
        assert!(!gen.ascii_header);

        let tts = template.tts_generator();
        assert_eq!(tts.rate, 1.0);
    }

    #[tokio::test]
    async fn data_aggregator_caching() {
        let aggregator = DataAggregator::builder()
            .comms_source(Arc::new(MockCommsSource))
            .econ_source(Arc::new(MockEconSource))
            .hackathon_source(Arc::new(MockHackathonSource))
            .metrics_source(Arc::new(MockMetricsSource))
            .headlines_source(Arc::new(MockHeadlinesSource))
            .build()
            .unwrap();

        let date = Utc.with_ymd_and_hms(2026, 1, 15, 6, 0, 0).unwrap();

        // First fetch
        let _ = aggregator.fetch_headlines(date).await.unwrap();
        // Second fetch should use cache
        let _ = aggregator.fetch_headlines(date).await.unwrap();

        // Cache should have entry
        let cache = aggregator.cache.read().await;
        assert!(cache.contains_key("headlines_20260115"));
    }
}