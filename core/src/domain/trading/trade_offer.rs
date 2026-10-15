/// TRADE OFFER:
/// 1. Amount of resource (maximum)
/// 2. Price per unit (minimum) 

#[derive(Debug, Clone, Copy)]
pub struct TradeOffer {
    pub amount_to_sell: u32,
    pub min_price_per_unit: u32,
}

impl TradeOffer {
    pub fn new(amount_to_sell: u32, min_price_per_unit: u32) -> TradeOffer {
        TradeOffer { amount_to_sell, min_price_per_unit }
    }
}