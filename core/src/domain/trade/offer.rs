/// TRADE OFFER
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeOffer {
    Buy(BuyTradeOffer),
    Sell(SellTradeOffer),
}
impl TradeOffer {
    /// Offer to sell `units` for at least `minimal_price_per_unit` gold each.
    pub fn sell(units: u32, minimal_price_per_unit: u32) -> Self {
        TradeOffer::Sell(SellTradeOffer::new(units, minimal_price_per_unit))
    }

    /// Offer to buy `units` for at most `maximum_price_per_unit` gold each.
    pub fn buy(units: u32, maximum_price_per_unit: u32) -> Self {
        TradeOffer::Buy(BuyTradeOffer::new(units, maximum_price_per_unit))
    }

    pub fn calculate_demand(&self) -> TradeDemand {
        match self {
            TradeOffer::Buy(buy_offer) => buy_offer.calculate_demand(),
            TradeOffer::Sell(sell_offer) => sell_offer.calculate_demand(),
        }
    }

    /// Units still open: stock left to sell, or quantity left to buy.
    pub fn remaining_units(&self) -> u32 {
        match self {
            TradeOffer::Buy(buy_offer) => buy_offer.available_units,
            TradeOffer::Sell(sell_offer) => sell_offer.available_units,
        }
    }

    /// Same open units and price limit, without units already bought.
    pub(crate) fn without_progress(&self) -> TradeOffer {
        match self {
            TradeOffer::Sell(sell_offer) => TradeOffer::Sell(sell_offer.clone()),
            TradeOffer::Buy(buy_offer) => {
                TradeOffer::buy(buy_offer.available_units, buy_offer.maximum_price_per_unit)
            }
        }
    }

    /// Moves up to `units` open units into a new offer with the same price limit.
    pub(crate) fn take_share(&mut self, units: u32) -> TradeOffer {
        match self {
            TradeOffer::Sell(sell_offer) => {
                let units = units.min(sell_offer.available_units);
                sell_offer.available_units -= units;
                TradeOffer::sell(units, sell_offer.minimal_price_per_unit)
            }
            TradeOffer::Buy(buy_offer) => {
                let units = units.min(buy_offer.available_units);
                buy_offer.available_units -= units;
                TradeOffer::buy(units, buy_offer.maximum_price_per_unit)
            }
        }
    }

    /// Merges a share created by `take_share` back into this offer.
    pub(crate) fn absorb(&mut self, share: TradeOffer) {
        match (self, share) {
            (TradeOffer::Sell(offer), TradeOffer::Sell(share)) => {
                offer.available_units += share.available_units;
            }
            (TradeOffer::Buy(offer), TradeOffer::Buy(share)) => {
                offer.available_units += share.available_units;
                offer.already_bought_units += share.already_bought_units;
            }
            _ => unreachable!("a share always keeps the kind of its source offer"),
        }
    }
}

/// TRADE DEMAND
pub enum TradeDemand {
    Gold(u64),
    ResourceType(u32),
}

/// SELL TRADE OFFER
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellTradeOffer {
    available_units: u32,
    minimal_price_per_unit: u32,
}

impl SellTradeOffer {
    pub fn new(available_units: u32, minimal_price_per_unit: u32) -> Self {
        Self {
            available_units,
            minimal_price_per_unit,
        }
    }
    fn calculate_demand(&self) -> TradeDemand {
        TradeDemand::ResourceType(self.available_units)
    }
    pub fn update_sell_resource_amount(&mut self, avaiable_units: u32) {
        self.available_units = avaiable_units;
    }
    pub fn get_amount_to_sell(&self) -> u32 {
        self.available_units
    }
    pub fn consume_avaiable_units(&mut self) -> u32 {
        std::mem::take(&mut self.available_units)
    }
    pub fn get_minimal_price_per_unit(&self) -> u32 {
        self.minimal_price_per_unit
    }
    pub fn subtract_sold_amount(&mut self, units: u32) {
        self.available_units -= units;
    }
}

/// BUY TRADE OFFER
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuyTradeOffer {
    already_bought_units: u32,
    available_units: u32,
    maximum_price_per_unit: u32,
}

impl BuyTradeOffer {
    pub fn new(units_to_buy: u32, maximum_price_per_unit: u32) -> Self {
        Self {
            already_bought_units: 0,
            available_units: units_to_buy,
            maximum_price_per_unit,
        }
    }
    fn calculate_demand(&self) -> TradeDemand {
        let gold_amount = u64::from(self.maximum_price_per_unit) * u64::from(self.available_units);
        TradeDemand::Gold(gold_amount)
    }
    pub fn consume_bought_units(&mut self) -> u32 {
        std::mem::take(&mut self.already_bought_units)
    }
    pub fn get_already_bought_units(&self) -> u32 {
        self.already_bought_units
    }
    pub fn get_maximum_price_per_unit(&self) -> u32 {
        self.maximum_price_per_unit
    }
    pub fn get_remaining_amount_to_buy(&self) -> u32 {
        self.available_units
    }
    pub fn add_bought_amount(&mut self, units: u32) {
        self.already_bought_units += units;
        self.available_units -= units;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::ResourceCatalog;
    use crate::domain::trade::book::TradeBook;
    use crate::test_support::material;
    use std::collections::HashMap;

    #[test]
    fn shares_return_to_their_source_offer_without_losing_units() {
        let mut sell = TradeOffer::sell(10, 3);
        let share = sell.take_share(4);
        assert_eq!(share, TradeOffer::sell(4, 3));
        assert_eq!(sell.remaining_units(), 6);
        sell.absorb(share);
        assert_eq!(sell, TradeOffer::sell(10, 3));

        let mut buy = TradeOffer::buy(5, 7);
        let mut share = buy.take_share(9);
        assert_eq!(buy.remaining_units(), 0);
        if let TradeOffer::Buy(offer) = &mut share {
            offer.add_bought_amount(2);
        }
        buy.absorb(share);
        let TradeOffer::Buy(offer) = buy else {
            unreachable!()
        };
        assert_eq!(offer.get_remaining_amount_to_buy(), 3);
        assert_eq!(offer.get_already_bought_units(), 2);
    }

    #[test]
    fn buy_demand_can_exceed_the_old_gold_counter_limit() {
        let TradeDemand::Gold(gold) = TradeOffer::buy(u32::MAX, 2).calculate_demand() else {
            unreachable!()
        };
        assert_eq!(gold, 2 * u64::from(u32::MAX));
    }

    #[test]
    fn trade_limits_and_gold_use_whole_units_for_coal_from_either_source() {
        let catalog = ResourceCatalog::standard();
        let seller_resource = catalog.resolve("charcoal").unwrap();
        let buyer_resource = catalog.resolve("mined_coal").unwrap();
        let mut seller = TradeBook::new(
            0,
            HashMap::from([(seller_resource, TradeOffer::sell(3, 3))]),
        );
        let mut buyer =
            TradeBook::new(15, HashMap::from([(buyer_resource, TradeOffer::buy(2, 5))]));

        assert_eq!(
            seller.calculate_demand(),
            (HashMap::from([(material("coal"), 3)]), 0)
        );
        assert_eq!(buyer.calculate_demand(), (HashMap::new(), 10));
        buyer.trade(&mut seller);
        assert_eq!(
            buyer.return_resources(),
            (HashMap::from([(material("coal"), 2)]), 7)
        );
        assert_eq!(
            seller.return_resources(),
            (HashMap::from([(material("coal"), 1)]), 8)
        );
    }
}
