//! Treasury, market offers and caravan orders.
use super::super::Settlement;
use super::super::trading::CaravanOrder;
use crate::domain::buildings::passive::TradeLimitBonuses;
use crate::domain::economy::ResourceType;
use crate::domain::ids::SettlementId;
use crate::domain::intelligence::{Information, InformationSnapshot};
use crate::domain::trade::book::TradeBook;
use crate::domain::trade::caravan::{CaravanCargo, CaravanReturn};
use crate::domain::trade::offer::TradeOffer;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeCommandError {
    /// Labour is a per-turn capacity, not a tradable good.
    LabourIsNotTradable,
    TradeWithSelf,
    EmptyOrder,
    UnknownMaterial(ResourceType),
    MarketCapacityExceeded,
    CaravanCapacityExceeded,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct CaravanUsage {
    pub slots: u32,
    pub units: u64,
}

impl CaravanUsage {
    pub fn add(&mut self, units: u64) {
        self.slots = self.slots.saturating_add(1);
        self.units = self.units.saturating_add(units);
    }

    pub fn fits(self, limits: TradeLimitBonuses) -> bool {
        self.slots <= limits.caravan_slots && self.units <= u64::from(limits.caravan_cargo)
    }
}

pub(crate) fn offer_units(offers: &HashMap<ResourceType, TradeOffer>) -> u64 {
    offers
        .values()
        .map(|offer| u64::from(offer.remaining_units()))
        .sum()
}

impl Settlement {
    pub fn set_base_trade_limits(&mut self, limits: Option<TradeLimitBonuses>) {
        self.trade.base_limits = limits;
    }

    pub(crate) fn set_active_caravan_usage(&mut self, usage: CaravanUsage) {
        self.trade.active_caravans = usage;
    }

    pub fn trade_limits(&self) -> Option<TradeLimitBonuses> {
        self.trade.base_limits.map(|base| {
            let bonus = self.trade_limit_bonuses();
            TradeLimitBonuses {
                caravan_slots: base.caravan_slots.saturating_add(bonus.caravan_slots),
                offer_slots: base.offer_slots.saturating_add(bonus.offer_slots),
                caravan_cargo: base.caravan_cargo.saturating_add(bonus.caravan_cargo),
                open_offer_units: base.open_offer_units.saturating_add(bonus.open_offer_units),
            }
        })
    }

    pub(crate) fn queued_caravan_usage(&self, mut usage: CaravanUsage) -> CaravanUsage {
        for order in &self.trade.orders {
            usage.add(offer_units(&order.offers));
        }
        usage
    }

    pub(crate) fn market_fits(&self, offers: &HashMap<ResourceType, TradeOffer>) -> bool {
        self.trade_limits().is_none_or(|limits| {
            offers
                .values()
                .filter(|offer| offer.remaining_units() > 0)
                .count()
                <= limits.offer_slots as usize
                && offer_units(offers) <= u64::from(limits.open_offer_units)
        })
    }

    pub fn deposit_gold(&mut self, amount: u32) {
        self.gold_inventory.add(amount);
    }

    /// Replaces the market offers. They stay published until replaced and are
    /// funded again, in full, at the start of every turn.
    pub fn publish_trade_offers(
        &mut self,
        offers: HashMap<ResourceType, TradeOffer>,
    ) -> Result<(), TradeCommandError> {
        self.validate_offers(&offers)?;
        if !self.market_fits(&offers) {
            return Err(TradeCommandError::MarketCapacityExceeded);
        }
        self.trade.market_offers = Self::without_progress(offers);
        Ok(())
    }

    pub fn clear_trade_offers(&mut self) {
        self.trade.market_offers.clear();
    }

    /// Queues a caravan funded from stock and gold at the start of the next turn.
    pub fn order_caravan(
        &mut self,
        receiver_id: SettlementId,
        offers: HashMap<ResourceType, TradeOffer>,
    ) -> Result<(), TradeCommandError> {
        if receiver_id == self.id {
            return Err(TradeCommandError::TradeWithSelf);
        }
        if offers.values().all(|offer| offer.remaining_units() == 0) {
            return Err(TradeCommandError::EmptyOrder);
        }
        self.validate_offers(&offers)?;
        let mut usage = self.queued_caravan_usage(self.trade.active_caravans);
        usage.add(offer_units(&offers));
        if self
            .trade_limits()
            .is_some_and(|limits| !usage.fits(limits))
        {
            return Err(TradeCommandError::CaravanCapacityExceeded);
        }
        self.trade.orders.push(CaravanOrder {
            receiver_id,
            offers: Self::without_progress(offers),
        });
        Ok(())
    }

    pub fn cancel_caravan_orders(&mut self) {
        self.trade.orders.clear();
    }

    /// Takes caravans funded in the last turn so a coordinator can route them.
    pub fn take_dispatched_caravans(&mut self) -> Vec<CaravanCargo> {
        std::mem::take(&mut self.trade.dispatched)
    }

    /// Accepts a returning caravan; it is unloaded and reported in the next turn.
    pub(crate) fn receive_caravan(&mut self, caravan: CaravanReturn) {
        self.trade.returning.push(caravan);
    }

    pub(in crate::domain::settlement) fn remember_observation(
        &mut self,
        snapshot: InformationSnapshot,
    ) {
        match snapshot.information() {
            Information::ForeignMarket { market, .. } => {
                self.trade.remember_market(*market, snapshot)
            }
        }
    }

    fn validate_offers(
        &self,
        offers: &HashMap<ResourceType, TradeOffer>,
    ) -> Result<(), TradeCommandError> {
        if offers.contains_key(&ResourceType::Labour) {
            return Err(TradeCommandError::LabourIsNotTradable);
        }
        if let Some(resource) = offers
            .keys()
            .filter(|resource| !self.knows_material(resource))
            .min()
        {
            return Err(TradeCommandError::UnknownMaterial(resource.clone()));
        }
        Ok(())
    }

    /// Keeps only the order parameters, so purchases made earlier on a copied
    /// offer cannot be collected again as goods from nowhere.
    fn without_progress(
        offers: HashMap<ResourceType, TradeOffer>,
    ) -> HashMap<ResourceType, TradeOffer> {
        offers
            .into_iter()
            .map(|(resource, offer)| (resource, offer.without_progress()))
            .collect()
    }
}

/// Read-only trade views.
impl Settlement {
    pub fn gold(&self) -> u32 {
        self.gold_inventory.amount()
    }

    /// Latest delivered snapshot for each foreign market, in ascending settlement ID.
    pub fn known_markets(&self) -> impl Iterator<Item = &InformationSnapshot> {
        self.trade.known_markets()
    }

    pub fn market_offers(&self) -> &HashMap<ResourceType, TradeOffer> {
        &self.trade.market_offers
    }

    /// Funded market state; empty between turns because it is drained every turn.
    pub fn trade_book(&self) -> &TradeBook {
        &self.book
    }

    pub fn caravan_orders(&self) -> &[CaravanOrder] {
        &self.trade.orders
    }

    pub fn returning_caravans(&self) -> &[CaravanReturn] {
        &self.trade.returning
    }
}
