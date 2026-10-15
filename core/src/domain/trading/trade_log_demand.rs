use crate::domain::{economy::resource::ResourceType, trading::trade_offer::TradeOffer};

pub struct TradeLogDemand {
    pub resource_type: ResourceType,
    pub trade_offer: TradeOffer,
}

impl TradeLogDemand {
    pub fn new(resource_type: ResourceType, trade_offer: TradeOffer) -> TradeLogDemand {
        TradeLogDemand { resource_type, trade_offer }
    }
}
