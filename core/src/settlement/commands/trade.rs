//! Treasury, market offers and caravan orders.
use super::super::Settlement;
use super::super::trade_manager::CaravanOrder;
use crate::economy::ResourceType;
use crate::trade::book::TradeBook;
use crate::trade::caravan::{CaravanCargo, CaravanReturn};
use crate::trade::offer::TradeOffer;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeCommandError {
    /// Labour is a per-turn capacity, not a tradable good.
    LabourIsNotTradable,
    TradeWithSelf,
    EmptyOrder,
}

impl Settlement {
    pub fn deposit_gold(&mut self, amount: u32) {
        self.gold_inventory.add(amount);
    }

    /// Replaces the market offers. They stay published until replaced and are
    /// funded again, in full, at the start of every turn.
    pub fn publish_trade_offers(
        &mut self,
        offers: HashMap<ResourceType, TradeOffer>,
    ) -> Result<(), TradeCommandError> {
        Self::validate_offers(&offers)?;
        self.trade.market_offers = Self::without_progress(offers);
        Ok(())
    }

    pub fn clear_trade_offers(&mut self) {
        self.trade.market_offers.clear();
    }

    /// Queues a caravan funded from stock and gold at the start of the next turn.
    pub fn order_caravan(
        &mut self,
        receiver_id: u32,
        offers: HashMap<ResourceType, TradeOffer>,
    ) -> Result<(), TradeCommandError> {
        if receiver_id == self.id {
            return Err(TradeCommandError::TradeWithSelf);
        }
        if offers.values().all(|offer| offer.remaining_units() == 0) {
            return Err(TradeCommandError::EmptyOrder);
        }
        Self::validate_offers(&offers)?;
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
    pub fn receive_caravan(&mut self, caravan: CaravanReturn) {
        self.trade.returning.push(caravan);
    }

    fn validate_offers(
        offers: &HashMap<ResourceType, TradeOffer>,
    ) -> Result<(), TradeCommandError> {
        if offers.contains_key(&ResourceType::Labour) {
            return Err(TradeCommandError::LabourIsNotTradable);
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
