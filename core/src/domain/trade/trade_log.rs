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
                    *returned_resources.entry(*resource_type).or_insert(0) += bought;
                }
                TradeOffer::Sell(sell_trade_offer) => {
                    let remained: u32 = sell_trade_offer.consume_avaiable_units();
                    *returned_resources.entry(*resource_type).or_insert(0) += remained;
                }
            }
        }

        (returned_resources, returned_gold)
    }

    pub fn trade(&mut self, other: &mut TradeLog) {
        // (ResourceType, price_per_unit, max_possible_units)
        let mut my_buying_trade_vector: Vec<(ResourceType, u32, u32)> = Vec::new();
        let mut my_total_cost: u32 = 0;
        let mut other_buying_trade_vector: Vec<(ResourceType, u32, u32)> = Vec::new();
        let mut other_total_cost: u32 = 0;

        for (resource_type, trade_offer) in &self.log {
            match trade_offer {
                TradeOffer::Buy(buy_trade_offer) => {
                    if let Some(TradeOffer::Sell(sell_trade_offer)) = other.log.get(resource_type) {
                        let price_min = sell_trade_offer.get_minimal_price_per_unit();
                        let price_max = buy_trade_offer.get_maximum_price_per_unit();
                        if price_min <= price_max {
                            let price_per_unit = (price_max + price_min) / 2;
                            let max_units = buy_trade_offer
                                .get_remaining_amount_to_buy()
                                .min(sell_trade_offer.get_amount_to_sell());

                            if max_units > 0 && price_per_unit > 0 {
                                my_total_cost += price_per_unit * max_units;
                                my_buying_trade_vector.push((
                                    *resource_type,
                                    price_per_unit,
                                    max_units,
                                ));
                            }
                        }
                    }
                }
                TradeOffer::Sell(sell_trade_offer) => {
                    if let Some(TradeOffer::Buy(buy_trade_offer)) = other.log.get(resource_type) {
                        let price_min = sell_trade_offer.get_minimal_price_per_unit();
                        let price_max = buy_trade_offer.get_maximum_price_per_unit();
                        if price_min <= price_max {
                            let price_per_unit = (price_max + price_min) / 2;
                            let max_units = buy_trade_offer
                                .get_remaining_amount_to_buy()
                                .min(sell_trade_offer.get_amount_to_sell());

                            if max_units > 0 && price_per_unit > 0 {
                                other_total_cost += price_per_unit * max_units;
                                other_buying_trade_vector.push((
                                    *resource_type,
                                    price_per_unit,
                                    max_units,
                                ));
                            }
                        }
                    }
                }
            }
        }

        let other_earned_gold =
            Self::transaction(my_buying_trade_vector, self, other, my_total_cost);
        let my_earned_gold =
            Self::transaction(other_buying_trade_vector, other, self, other_total_cost);

        self.gold += my_earned_gold;
        other.gold += other_earned_gold;
    }

    fn transaction(
        mut buying_trade_vector: Vec<(ResourceType, u32, u32)>,
        buy_log: &mut TradeLog,
        sell_log: &mut TradeLog,
        total_cost: u32,
    ) -> u32 {
        if total_cost == 0 || buy_log.gold == 0 {
            return 0;
        }

        buying_trade_vector.sort_by_key(|b| std::cmp::Reverse(b.1));

        let initial_gold: u32 = buy_log.gold;
        let mut total_gold_spent: u32 = 0;

        for &mut (resource, price_per_unit, ref mut remaining_units) in &mut buying_trade_vector {
            let units_to_buy = if initial_gold >= total_cost {
                *remaining_units
            } else {
                ((*remaining_units as u64) * (initial_gold as u64) / (total_cost as u64)) as u32
            };

            if units_to_buy > 0 {
                Self::execute_single_trade(
                    buy_log,
                    sell_log,
                    resource,
                    units_to_buy,
                    price_per_unit,
                );
                *remaining_units -= units_to_buy;
                total_gold_spent += units_to_buy * price_per_unit;
            }
        }

        if initial_gold < total_cost && buy_log.gold > 0 {
            for (resource, price_per_unit, remaining_units) in buying_trade_vector {
                if remaining_units == 0 || buy_log.gold < price_per_unit {
                    continue;
                }
                let affordable_extra = (buy_log.gold / price_per_unit).min(remaining_units);
                if affordable_extra > 0 {
                    Self::execute_single_trade(
                        buy_log,
                        sell_log,
                        resource,
                        affordable_extra,
                        price_per_unit,
                    );
                    total_gold_spent += affordable_extra * price_per_unit;
                }
            }
        }

        total_gold_spent
    }

    fn execute_single_trade(
        buy_log: &mut TradeLog,
        sell_log: &mut TradeLog,
        resource: ResourceType,
        units: u32,
        price: u32,
    ) {
        let cost = units * price;
        buy_log.gold -= cost;

        if let Some(TradeOffer::Buy(buy_offer)) = buy_log.log.get_mut(&resource) {
            buy_offer.add_bought_amount(units);
        }

        if let Some(TradeOffer::Sell(sell_offer)) = sell_log.log.get_mut(&resource) {
            sell_offer.subtract_sold_amount(units);
        }
    }
}

impl Default for TradeLog {
    fn default() -> Self {
        let gold: u32 = 0;
        let log: HashMap<ResourceType, TradeOffer> = HashMap::new();
        Self::new(gold, log)
    }
}
