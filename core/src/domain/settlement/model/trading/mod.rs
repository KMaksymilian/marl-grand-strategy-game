//! Published market offers, caravan orders, funded departures and returns waiting to be unloaded.
mod lifecycle;

use crate::domain::economy::ResourceType;
use crate::domain::ids::SettlementId;
use crate::domain::intelligence::InformationSnapshot;
use crate::domain::trade::book::TradeBook;
use crate::domain::trade::caravan::{CaravanCargo, CaravanReturn};
use crate::domain::trade::offer::TradeOffer;
use std::collections::{BTreeMap, HashMap, btree_map::Entry};

/// Funded caravan waiting for the end-of-turn departure phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaravanOrder {
    pub receiver_id: SettlementId,
    pub offers: HashMap<ResourceType, TradeOffer>,
    gold: u64,
}

impl CaravanOrder {
    pub(super) fn funded(
        receiver_id: SettlementId,
        offers: HashMap<ResourceType, TradeOffer>,
        gold: u64,
    ) -> Self {
        Self {
            receiver_id,
            offers,
            gold,
        }
    }

    pub(super) fn into_cargo(self, sender_id: SettlementId) -> CaravanCargo {
        CaravanCargo {
            sender_id,
            receiver_id: self.receiver_id,
            book: TradeBook::new(self.gold, self.offers),
        }
    }
}

#[derive(Debug, Default)]
pub(in crate::domain::settlement) struct TradeManager {
    /// Copied into the market book and funded at the start of every turn.
    pub(in crate::domain::settlement) market_offers: HashMap<ResourceType, TradeOffer>,
    /// Funds withdrawn at acceptance; released for departure in queue order.
    pub(in crate::domain::settlement) orders: Vec<CaravanOrder>,
    /// Due for departure this turn; collected by the world coordinator.
    pub(in crate::domain::settlement) dispatched: Vec<CaravanCargo>,
    /// Unloaded into the warehouse during the next turn.
    pub(in crate::domain::settlement) returning: Vec<CaravanReturn>,
    /// Latest delivered observation for each foreign market.
    known_markets: BTreeMap<SettlementId, InformationSnapshot>,
    /// Latest troop observation of each province (MGS-45): from caravans and
    /// from the settlement's own watch.
    known_troops: BTreeMap<usize, InformationSnapshot>,
}

impl TradeManager {
    pub(super) fn escrowed_goods(&self) -> impl Iterator<Item = (&ResourceType, u32)> {
        self.orders.iter().flat_map(|order| {
            order
                .offers
                .iter()
                .filter_map(|(resource, offer)| match offer {
                    TradeOffer::Sell(sell) => Some((resource, sell.get_amount_to_sell())),
                    TradeOffer::Buy(_) => None,
                })
        })
    }

    pub(super) fn has_escrowed_gold(&self) -> bool {
        self.orders.iter().any(|order| order.gold > 0)
    }

    pub(super) fn known_troops(&self) -> impl Iterator<Item = &InformationSnapshot> {
        self.known_troops.values()
    }

    /// A newer or equally recent observation replaces the stored one.
    pub(super) fn remember_troops(&mut self, province_id: usize, snapshot: InformationSnapshot) {
        match self.known_troops.entry(province_id) {
            Entry::Occupied(entry) if snapshot.observed_turn() < entry.get().observed_turn() => {}
            entry => {
                let _ = entry.insert_entry(snapshot);
            }
        }
    }

    pub(super) fn known_markets(&self) -> impl Iterator<Item = &InformationSnapshot> {
        self.known_markets.values()
    }

    pub(super) fn remember_market(&mut self, market: SettlementId, snapshot: InformationSnapshot) {
        match self.known_markets.entry(market) {
            Entry::Vacant(entry) => {
                entry.insert(snapshot);
            }
            Entry::Occupied(mut entry)
                if snapshot.observed_turn() > entry.get().observed_turn() =>
            {
                entry.insert(snapshot);
            }
            Entry::Occupied(_) => {}
        }
    }
}
