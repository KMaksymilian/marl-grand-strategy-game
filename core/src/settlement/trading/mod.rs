//! Published market offers, caravan orders, funded departures and returns waiting to be unloaded.
mod lifecycle;

use crate::economy::ResourceType;
use crate::ids::SettlementId;
use crate::trade::caravan::{CaravanCargo, CaravanReturn};
use crate::trade::offer::TradeOffer;
use std::collections::HashMap;

/// Request to fund and send one caravan in the next turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaravanOrder {
    pub receiver_id: SettlementId,
    pub offers: HashMap<ResourceType, TradeOffer>,
}

#[derive(Debug, Default)]
pub(in crate::settlement) struct TradeManager {
    /// Copied into the market book and funded at the start of every turn.
    pub(in crate::settlement) market_offers: HashMap<ResourceType, TradeOffer>,
    /// Funded during the next turn, in queue order, after the market book.
    pub(in crate::settlement) orders: Vec<CaravanOrder>,
    /// Funded this turn; collected by the world coordinator.
    pub(in crate::settlement) dispatched: Vec<CaravanCargo>,
    /// Unloaded into the warehouse during the next turn.
    pub(in crate::settlement) returning: Vec<CaravanReturn>,
}
