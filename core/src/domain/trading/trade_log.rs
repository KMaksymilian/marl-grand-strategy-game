use crate::domain::{economy::resource::ResourceType, trading::{trade_log_demand::TradeLogDemand, trade_offer::TradeOffer}};
use std::collections::HashMap;

#[derive(Debug)]
pub struct TradeLog {
    log: HashMap<ResourceType, TradeOffer>,
}

impl TradeLog {
    pub fn new() -> TradeLog {
        let log: HashMap<ResourceType, TradeOffer> = HashMap::new();
        TradeLog { log }
    }
    pub fn add_or_update_trade_offer(&mut self, resource_type: ResourceType, amount_to_sell: u32, min_price_per_unit: u32) {
        let trade_offer: TradeOffer = TradeOffer::new(amount_to_sell, min_price_per_unit);
        self.log.insert(resource_type, trade_offer);
    }
    pub fn generate_trade_log_demand(&self) -> Vec<TradeLogDemand> {
        let mut demands: Vec<TradeLogDemand> = Vec::new();
        for (resource_type, trade_offer) in &self.log {
            demands.push(TradeLogDemand::new(*resource_type, *trade_offer));
        }
        demands
    }
}
