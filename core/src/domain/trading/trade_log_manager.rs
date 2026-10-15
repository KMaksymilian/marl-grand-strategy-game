use crate::domain::trading::{trade_log::TradeLog, trade_log_demand::TradeLogDemand};

#[derive(Debug)]
pub struct TradeLogManager {
    pub trade_log: TradeLog,
    pub demand_log: TradeLog,
}

impl TradeLogManager {
    pub fn new() -> TradeLogManager {
        let trade_log: TradeLog = TradeLog::new();
        let demand_log: TradeLog = TradeLog::new();
        TradeLogManager { trade_log, demand_log }
    }
    pub fn generate_trade_log_demand(&self) -> Vec<TradeLogDemand> {
        self.demand_log.generate_trade_log_demand()
    }
}