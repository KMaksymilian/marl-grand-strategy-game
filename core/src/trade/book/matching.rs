//! Bilateral offer matching, funding distribution and transaction execution.
use super::TradeBook;
use crate::economy::ResourceType;
use crate::trade::offer::TradeOffer;

impl TradeBook {
    pub fn trade(&mut self, other: &mut TradeBook) {
        // (ResourceType, price_per_unit, max_possible_units)
        let mut my_buying_trade_vector: Vec<(ResourceType, u32, u32)> = Vec::new();
        let mut my_total_cost: u32 = 0;
        let mut other_buying_trade_vector: Vec<(ResourceType, u32, u32)> = Vec::new();
        let mut other_total_cost: u32 = 0;

        for (resource_type, offer) in &self.offers {
            match offer {
                TradeOffer::Buy(buy_offer) => {
                    if let Some(TradeOffer::Sell(sell_offer)) = other.offers.get(resource_type) {
                        let price_min = sell_offer.get_minimal_price_per_unit();
                        let price_max = buy_offer.get_maximum_price_per_unit();
                        if price_min <= price_max {
                            let price_per_unit = (price_max + price_min) / 2;
                            let max_units = buy_offer
                                .get_remaining_amount_to_buy()
                                .min(sell_offer.get_amount_to_sell());

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
                TradeOffer::Sell(sell_offer) => {
                    if let Some(TradeOffer::Buy(buy_offer)) = other.offers.get(resource_type) {
                        let price_min = sell_offer.get_minimal_price_per_unit();
                        let price_max = buy_offer.get_maximum_price_per_unit();
                        if price_min <= price_max {
                            let price_per_unit = (price_max + price_min) / 2;
                            let max_units = buy_offer
                                .get_remaining_amount_to_buy()
                                .min(sell_offer.get_amount_to_sell());

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
        buy_book: &mut TradeBook,
        sell_book: &mut TradeBook,
        total_cost: u32,
    ) -> u32 {
        if total_cost == 0 || buy_book.gold == 0 {
            return 0;
        }

        buying_trade_vector.sort_by_key(|b| std::cmp::Reverse(b.1));

        let initial_gold: u32 = buy_book.gold;
        let mut total_gold_spent: u32 = 0;

        for &mut (resource, price_per_unit, ref mut remaining_units) in &mut buying_trade_vector {
            let units_to_buy = if initial_gold >= total_cost {
                *remaining_units
            } else {
                ((*remaining_units as u64) * (initial_gold as u64) / (total_cost as u64)) as u32
            };

            if units_to_buy > 0 {
                Self::execute_single_trade(
                    buy_book,
                    sell_book,
                    resource,
                    units_to_buy,
                    price_per_unit,
                );
                *remaining_units -= units_to_buy;
                total_gold_spent += units_to_buy * price_per_unit;
            }
        }

        if initial_gold < total_cost && buy_book.gold > 0 {
            for (resource, price_per_unit, remaining_units) in buying_trade_vector {
                if remaining_units == 0 || buy_book.gold < price_per_unit {
                    continue;
                }
                let affordable_extra = (buy_book.gold / price_per_unit).min(remaining_units);
                if affordable_extra > 0 {
                    Self::execute_single_trade(
                        buy_book,
                        sell_book,
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
        buy_book: &mut TradeBook,
        sell_book: &mut TradeBook,
        resource: ResourceType,
        units: u32,
        price: u32,
    ) {
        let cost = units * price;
        buy_book.gold -= cost;

        if let Some(TradeOffer::Buy(buy_offer)) = buy_book.offers.get_mut(&resource) {
            buy_offer.add_bought_amount(units);
        }

        if let Some(TradeOffer::Sell(sell_offer)) = sell_book.offers.get_mut(&resource) {
            sell_offer.subtract_sold_amount(units);
        }
    }
}
