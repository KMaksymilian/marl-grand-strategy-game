/// TRADE OFFER
#[derive(Debug)]
pub enum TradeOffer {
    Buy(BuyTradeOffer),
    Sell(SellTradeOffer),
}
impl TradeOffer {
    pub fn calculate_demand(&self) -> TradeDemand {
        match self {
            TradeOffer::Buy(buy_trade_offer) => buy_trade_offer.calculate_demand(),
            TradeOffer::Sell(sell_trade_offer) => sell_trade_offer.calculate_demand(),
        }
    }
}

/// TRADE DEMAND
pub enum TradeDemand {
    Gold(u32),
    ResourceType(u32),
}

/// SELL TRADE OFFER
#[derive(Debug)]
pub struct SellTradeOffer {
    available_units: u32,
    minimal_price_per_unit: u32,
}

impl SellTradeOffer {
    fn calculate_demand(&self) -> TradeDemand {
        TradeDemand::ResourceType(self.available_units)
    }
    pub fn update_sell_resource_amount(&mut self, avaiable_units: u32) {
        self.available_units = avaiable_units;
    }
    pub fn consume_avaiable_units(&mut self) -> u32 {
        std::mem::take(&mut self.available_units)
    }
    pub fn get_minimal_price_per_unit(&self) -> u32 {
        self.minimal_price_per_unit
    }
}

/// BUY TRADE OFFER
#[derive(Debug)]
pub struct BuyTradeOffer {
    already_bought_units: u32,
    available_units: u32,
    maximum_price_per_unit: u32,
}

impl BuyTradeOffer {
    fn calculate_demand(&self) -> TradeDemand {
        let gold_amount: u32 = self.maximum_price_per_unit * self.available_units;
        TradeDemand::Gold(gold_amount)
    }
    pub fn consume_bought_units(&mut self) -> u32 {
        self.available_units += self.already_bought_units;
        std::mem::take(&mut self.already_bought_units)
    }
    pub fn get_maximum_price_per_unit(&self) -> u32 {
        self.maximum_price_per_unit
    }
}
