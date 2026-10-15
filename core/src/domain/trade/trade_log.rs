use crate::domain::{
    economy::resource::ResourceType,
    trade::trade_offer::{TradeDemand, TradeOffer},
};
use std::collections::HashMap;

/// TRADE LOG
#[derive(Debug)]
pub struct TradeLog {
    gold: u32,
    log: HashMap<ResourceType, TradeOffer>,
}

impl TradeLog {
    pub fn new(gold: u32, log: HashMap<ResourceType, TradeOffer>) -> Self {
        TradeLog { gold, log }
    }

    pub fn add_gold_amount(&mut self, gold_amount: u32) {
        self.gold += gold_amount;
    }

    pub fn update_sell_resource_amount(&mut self, res_type: ResourceType, avaiable_units: u32) {
        if let Some(trade_offer) = self.log.get_mut(&res_type)
            && let TradeOffer::Sell(sell_trade_offer) = trade_offer
        {
            sell_trade_offer.update_sell_resource_amount(avaiable_units);
        }
    }

    pub fn calculate_demand(&self) -> (HashMap<ResourceType, u32>, u32) {
        let mut resource_demands: HashMap<ResourceType, u32> = HashMap::new();
        let mut gold_demand: u32 = 0;

        for (resource_type, trade_offer) in &self.log {
            match trade_offer.calculate_demand() {
                TradeDemand::Gold(gold_amount) => gold_demand += gold_amount,
                TradeDemand::ResourceType(resource_amount) => {
                    *resource_demands.entry(*resource_type).or_default() += resource_amount;
                }
            }
        }

        (resource_demands, gold_demand)
    }

    pub fn return_resources(&mut self) -> (HashMap<ResourceType, u32>, u32) {
        let mut returned_resources: HashMap<ResourceType, u32> = HashMap::new();
        let returned_gold: u32 = std::mem::take(&mut self.gold);

        for (resource_type, trade_offer) in &mut self.log {
            match trade_offer {
                TradeOffer::Buy(buy_trade_offer) => {
                    let bought: u32 = buy_trade_offer.consume_bought_units();
                    *returned_resources.entry(*resource_type).or_insert(bought) += bought;
                }
                TradeOffer::Sell(sell_trade_offer) => {
                    let remained: u32 = sell_trade_offer.consume_avaiable_units();
                    *returned_resources.entry(*resource_type).or_insert(remained) += remained;
                }
            }
        }

        (returned_resources, returned_gold)
    }
}

impl Default for TradeLog {
    fn default() -> Self {
        let gold: u32 = 0;
        let log: HashMap<ResourceType, TradeOffer> = HashMap::new();
        Self::new(gold, log)
    }
}
