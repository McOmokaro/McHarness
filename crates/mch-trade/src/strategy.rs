//! Strategy engine and signal generation

use crate::{TradeError, Result};
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub symbol: String,
    pub action: SignalAction,
    pub strength: Decimal,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalAction {
    Buy,
    Sell,
    Hold,
}

pub struct StrategyEngine {
    strategies: Vec<Box<dyn Strategy>>,
}

pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    fn analyze(&self, market_data: &MarketData) -> Option<Signal>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub symbol: String,
    pub price: Decimal,
    pub volume: Decimal,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl StrategyEngine {
    pub fn new() -> Self {
        Self { strategies: Vec::new() }
    }

    pub fn add_strategy(&mut self, strategy: Box<dyn Strategy>) {
        self.strategies.push(strategy);
    }

    pub fn generate_signals(&self, market_data: &MarketData) -> Vec<Signal> {
        self.strategies.iter()
            .filter_map(|s| s.analyze(market_data))
            .collect()
    }
}

impl Default for StrategyEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SignalGenerator {
    engine: StrategyEngine,
}

impl SignalGenerator {
    pub fn new(engine: StrategyEngine) -> Self {
        Self { engine }
    }

    pub fn generate(&self, market_data: &MarketData) -> Vec<Signal> {
        self.engine.generate_signals(market_data)
    }
}