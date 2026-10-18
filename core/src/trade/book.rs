//! Offer state, reservations and resource collection.
mod matching;

use crate::{
    economy::ResourceType,
    trade::offer::{TradeDemand, TradeOffer},
};
use std::collections::HashMap;

/// Funded offers available for bilateral trade.
#[derive(Debug)]
pub struct TradeBook {
    gold: u32,
    offers: HashMap<ResourceType, TradeOffer>,
}

impl TradeBook {
    pub fn new(gold: u32, offers: HashMap<ResourceType, TradeOffer>) -> Self {
        TradeBook { gold, offers }
    }

    pub fn add_gold_amount(&mut self, gold_amount: u32) {
        self.gold += gold_amount;
    }

    pub fn update_sell_resource_amount(&mut self, res_type: ResourceType, avaiable_units: u32) {
        if let Some(offer) = self.offers.get_mut(&res_type)
            && let TradeOffer::Sell(sell_offer) = offer
        {
            sell_offer.update_sell_resource_amount(avaiable_units);
        }
    }

    pub fn calculate_demand(&self) -> (HashMap<ResourceType, u32>, u32) {
        let mut resource_demands: HashMap<ResourceType, u32> = HashMap::new();
        let mut gold_demand: u32 = 0;

        for (resource_type, offer) in &self.offers {
            match offer.calculate_demand() {
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

        for (resource_type, offer) in &mut self.offers {
            match offer {
                TradeOffer::Buy(buy_offer) => {
                    let bought: u32 = buy_offer.consume_bought_units();
                    *returned_resources.entry(*resource_type).or_insert(0) += bought;
                }
                TradeOffer::Sell(sell_offer) => {
                    let remained: u32 = sell_offer.consume_avaiable_units();
                    *returned_resources.entry(*resource_type).or_insert(0) += remained;
                }
            }
        }

        (returned_resources, returned_gold)
    }
}

impl Default for TradeBook {
    fn default() -> Self {
        let gold: u32 = 0;
        let offers: HashMap<ResourceType, TradeOffer> = HashMap::new();
        Self::new(gold, offers)
    }
}
