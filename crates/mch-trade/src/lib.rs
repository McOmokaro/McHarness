//! mch-trade — Binance API integration with mch-scope risk guards (stop-loss, max trade limits). Part of Overnight Agent economic engine.

pub mod binance;
pub mod risk;
pub mod strategy;
pub mod portfolio;

pub use binance::{BinanceClient, BinanceConfig};
pub use risk::{RiskGuard, StopLoss, PositionSizer, MaxTradeLimits};
pub use strategy::{StrategyEngine, SignalGenerator};
pub use portfolio::{PortfolioManager, Rebalancer};

#[derive(Debug, thiserror::Error)]
pub enum TradeError {
    #[error("Binance API error: {0}")]
    Binance(String),
    #[error("Risk management error: {0}")]
    Risk(String),
    #[error("Strategy error: {0}")]
    Strategy(String),
    #[error("Portfolio error: {0}")]
    Portfolio(String),
}

pub type Result<T> = std::result::Result<T, TradeError>;