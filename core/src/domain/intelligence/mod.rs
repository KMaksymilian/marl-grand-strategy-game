//! Snapshots of information collected by actors in the simulation.
mod market;
mod troops;

use crate::domain::{economy::ResourceType, ids::SettlementId};
use std::collections::BTreeMap;

pub use market::{MarketObservation, MarketObservationSource, MarketOffer, MarketOfferSide};
pub use troops::{TroopSighting, TroopSource};

/// A record of what was visible at a particular point in the simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InformationSnapshot {
    observed_turn: u64,
    information: Information,
}

impl InformationSnapshot {
    pub fn new(observed_turn: u64, information: Information) -> Self {
        Self {
            observed_turn,
            information,
        }
    }

    /// The turn when the information was collected, even if delivered later.
    pub fn observed_turn(&self) -> u64 {
        self.observed_turn
    }

    pub fn information(&self) -> &Information {
        &self.information
    }
}

/// Collects an owned snapshot from the data available to an observer.
pub trait ObservationStrategy<Source> {
    fn observe(&self, observed_turn: u64, source: Source) -> InformationSnapshot;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Information {
    ForeignMarket {
        market: SettlementId,
        province_id: usize,
        /// An empty map means the market was observed with no open offers.
        offers: BTreeMap<ResourceType, MarketOffer>,
    },
    /// The armies standing in a province (ADR 0006): every nation's living
    /// soldiers there, ascending nation ID; empty when no army stood there.
    TroopPresence {
        province_id: usize,
        source: TroopSource,
        troops: Vec<TroopSighting>,
    },
}
