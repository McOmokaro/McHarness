//! Risk management with stop-loss, position sizing, and max trade limits

use crate::{TradeError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_position_size_pct: Decimal,
    pub max_daily_loss_pct: Decimal,
    pub stop_loss_pct: Decimal,
    pub max_trades_per_day: u32,
    pub max_open_positions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: PositionSide,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionSide {
    Long,
    Short,
}

pub struct RiskGuard {
    config: RiskConfig,
    daily_pnl: Decimal,
    trades_today: u32,
    open_positions: Vec<Position>,
}

impl RiskGuard {
    pub fn new(config: RiskConfig) -> Self {
        Self {
            config,
            daily_pnl: Decimal::ZERO,
            trades_today: 0,
            open_positions: Vec::new(),
        }
    }

    pub fn check_new_position(&self, position: &Position) -> Result<()> {
        if self.open_positions.len() >= self.config.max_open_positions as usize {
            return Err(TradeError::Risk("Max open positions reached".to_string()));
        }
        if self.trades_today >= self.config.max_trades_per_day {
            return Err(TradeError::Risk("Max trades per day reached".to_string()));
        }
        if self.daily_pnl <= -self.config.max_daily_loss_pct {
            return Err(TradeError::Risk("Max daily loss reached".to_string()));
        }
        Ok(())
    }

    pub fn calculate_position_size(&self, account_balance: Decimal, risk_per_trade: Decimal) -> Decimal {
        account_balance * risk_per_trade
    }

    pub fn apply_stop_loss(&mut self, position: &mut Position, current_price: Decimal) -> bool {
        if let Some(stop_loss) = position.stop_loss {
            let should_close = match position.side {
                PositionSide::Long => current_price <= stop_loss,
                PositionSide::Short => current_price >= stop_loss,
            };
            if should_close {
                return true;
            }
        }
        false
    }
}

pub struct StopLoss {
    pct: Decimal,
}

impl StopLoss {
    pub fn new(pct: Decimal) -> Self {
        Self { pct }
    }

    pub fn calculate(&self, entry_price: Decimal, side: PositionSide) -> Decimal {
        match side {
            PositionSide::Long => entry_price * (Decimal::ONE - self.pct),
            PositionSide::Short => entry_price * (Decimal::ONE + self.pct),
        }
    }
}

pub struct PositionSizer {
    risk_per_trade: Decimal,
}

impl PositionSizer {
    pub fn new(risk_per_trade: Decimal) -> Self {
        Self { risk_per_trade }
    }

    pub fn size(&self, account_balance: Decimal, entry_price: Decimal, stop_loss: Decimal) -> Decimal {
        let risk_amount = account_balance * self.risk_per_trade;
        let risk_per_unit = (entry_price - stop_loss).abs();
        if risk_per_unit == Decimal::ZERO {
            Decimal::ZERO
        } else {
            risk_amount / risk_per_unit
        }
    }
}

pub struct MaxTradeLimits {
    max_daily: u32,
    max_open: u32,
}

impl MaxTradeLimits {
    pub fn new(max_daily: u32, max_open: u32) -> Self {
        Self { max_daily, max_open }
    }

    pub fn check_daily(&self, current: u32) -> bool {
        current < self.max_daily
    }

    pub fn check_open(&self, current: u32) -> bool {
        current < self.max_open
    }
}