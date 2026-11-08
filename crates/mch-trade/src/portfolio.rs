//! Portfolio management and rebalancing

use crate::{TradeError, Result};
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub positions: HashMap<String, Position>,
    pub cash: Decimal,
    pub total_value: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: Decimal,
    pub avg_entry_price: Decimal,
    pub current_price: Decimal,
    pub unrealized_pnl: Decimal,
}

pub struct PortfolioManager {
    portfolio: Portfolio,
    target_allocations: HashMap<String, Decimal>,
}

impl PortfolioManager {
    pub fn new(initial_cash: Decimal) -> Self {
        Self {
            portfolio: Portfolio {
                positions: HashMap::new(),
                cash: initial_cash,
                total_value: initial_cash,
            },
            target_allocations: HashMap::new(),
        }
    }

    pub fn update_position(&mut self, symbol: String, quantity: Decimal, price: Decimal) {
        let position = self.portfolio.positions.entry(symbol.clone()).or_insert(Position {
            symbol: symbol.clone(),
            quantity: Decimal::ZERO,
            avg_entry_price: Decimal::ZERO,
            current_price: price,
            unrealized_pnl: Decimal::ZERO,
        });
        
        let total_cost = position.avg_entry_price * position.quantity + price * quantity;
        let total_quantity = position.quantity + quantity;
        
        if total_quantity != Decimal::ZERO {
            position.avg_entry_price = total_cost / total_quantity;
        }
        position.quantity = total_quantity;
        position.current_price = price;
        position.unrealized_pnl = (price - position.avg_entry_price) * position.quantity;
        
        self.recalculate_total();
    }

    fn recalculate_total(&mut self) {
        self.portfolio.total_value = self.portfolio.cash + 
            self.portfolio.positions.values().map(|p| p.quantity * p.current_price).sum::<Decimal>();
    }

    pub fn get_total_value(&self) -> Decimal {
        self.portfolio.total_value
    }

    pub fn set_target_allocation(&mut self, symbol: String, pct: Decimal) {
        self.target_allocations.insert(symbol, pct);
    }
}

pub struct Rebalancer {
    manager: PortfolioManager,
}

impl Rebalancer {
    pub fn new(manager: PortfolioManager) -> Self {
        Self { manager }
    }

    pub fn rebalance(&mut self) -> Result<Vec<RebalanceAction>> {
        let total = self.manager.get_total_value();
        let mut actions = Vec::new();
        
        for (symbol, target_pct) in &self.manager.target_allocations {
            let target_value = total * *target_pct;
            let current_value = self.manager.portfolio.positions.get(symbol)
                .map(|p| p.quantity * p.current_price)
                .unwrap_or(Decimal::ZERO);
            
            let diff = target_value - current_value;
            if diff.abs() > Decimal::from(10) { // $10 threshold
                actions.push(RebalanceAction {
                    symbol: symbol.clone(),
                    action: if diff > Decimal::ZERO { RebalanceActionType::Buy } else { RebalanceActionType::Sell },
                    amount: diff.abs(),
                });
            }
        }
        
        Ok(actions)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceAction {
    pub symbol: String,
    pub action: RebalanceActionType,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebalanceActionType {
    Buy,
    Sell,
}