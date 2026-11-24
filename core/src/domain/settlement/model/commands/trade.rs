//! Treasury, market offers and caravan orders.
use super::super::Settlement;
use super::super::trading::CaravanOrder;
use crate::domain::buildings::passive::TradeLimitBonuses;
use crate::domain::economy::ResourceType;
use crate::domain::ids::SettlementId;
use crate::domain::intelligence::{Information, InformationSnapshot};
use crate::domain::trade::book::TradeBook;
use crate::domain::trade::caravan::{CaravanCargo, CaravanOutcome, CaravanReturn};
use crate::domain::trade::offer::TradeOffer;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeCommandError {
    /// Labour is a per-turn capacity, not a tradable good.
    LabourIsNotTradable,
    TradeWithSelf,
    UnknownSender(SettlementId),
    UnknownReceiver(SettlementId),
    EmptyOrder,
    UnknownMaterial(ResourceType),
    MarketCapacityExceeded,
    CaravanCapacityExceeded,
    ZeroUnits(ResourceType),
    NothingToCarry,
    InsufficientStock {
        resource: ResourceType,
        requested: u64,
        available: u32,
    },
    InsufficientGold {
        requested: u64,
        available: u32,
    },
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

    pub fn trade_limits(&self) -> Option<TradeLimitBonuses> {
        self.trade
            .base_limits
            .map(|base| base.saturating_add(self.trade_limit_bonuses()))
    }

    fn queued_caravan_usage(&self, mut usage: CaravanUsage) -> CaravanUsage {
        for order in &self.trade.orders {
            usage.add(offer_units(&order.offers));
        }
        for departure in &self.trade.dispatched {
            usage.add(offer_units(departure.book.offers()));
        }
        usage
    }

    fn market_fits(
        offers: &HashMap<ResourceType, TradeOffer>,
        limits: Option<TradeLimitBonuses>,
    ) -> bool {
        limits.is_none_or(|limits| {
            offers
                .values()
                .filter(|offer| offer.remaining_units() > 0)
                .count()
                <= limits.offer_slots as usize
                && offer_units(offers) <= u64::from(limits.open_offer_units)
        })
    }

    pub(in crate::domain::settlement) fn fundable_market_offers(
        &self,
    ) -> HashMap<ResourceType, TradeOffer> {
        let offers = self.trade.market_offers.clone();
        let Some(limits) = self.trade_limits() else {
            return offers;
        };
        let mut sorted: Vec<_> = offers.into_iter().collect();
        sorted.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let mut remaining = limits.open_offer_units;
        sorted
            .into_iter()
            .filter_map(|(resource, mut offer)| {
                let units = offer.remaining_units().min(remaining);
                if units == 0 {
                    return None;
                }
                remaining -= units;
                Some((resource, offer.take_share(units)))
            })
            .take(limits.offer_slots as usize)
            .collect()
    }

    pub fn deposit_gold(&mut self, amount: u32) {
        self.gold_inventory.add(amount);
    }

    /// Replaces the market offers. They stay published until replaced and are
    /// funded within current trade limits and available stock at the start of each turn.
    pub fn publish_trade_offers(
        &mut self,
        offers: HashMap<ResourceType, TradeOffer>,
    ) -> Result<(), TradeCommandError> {
        self.validate_offers(&offers)?;
        if !Self::market_fits(&offers, self.trade_limits()) {
            return Err(TradeCommandError::MarketCapacityExceeded);
        }
        if !self.trade.orders.is_empty() {
            let (goods, gold) = TradeBook::funding_demand(&offers);
            let escrowed: std::collections::HashSet<_> = self
                .trade
                .escrowed_goods()
                .map(|(resource, _)| resource)
                .collect();
            for (resource, requested) in goods {
                let available = self.inventory_manager.main.amount(resource.clone());
                if requested > available && escrowed.contains(&resource) {
                    return Err(TradeCommandError::InsufficientStock {
                        resource,
                        requested: u64::from(requested),
                        available,
                    });
                }
            }
            if self.trade.has_escrowed_gold() && gold > u64::from(self.gold()) {
                return Err(TradeCommandError::InsufficientGold {
                    requested: gold,
                    available: self.gold(),
                });
            }
        }
        self.trade.market_offers = Self::without_progress(offers);
        Ok(())
    }

    pub fn clear_trade_offers(&mut self) {
        self.trade.market_offers.clear();
    }

    /// Funds a caravan immediately from stock and gold left after the published market.
    /// Use the world-level command to require a registered receiver.
    pub fn order_caravan(
        &mut self,
        receiver_id: SettlementId,
        offers: HashMap<ResourceType, TradeOffer>,
    ) -> Result<(), TradeCommandError> {
        self.order_caravan_with_usage(receiver_id, offers, CaravanUsage::default())
    }

    pub(crate) fn order_caravan_with_usage(
        &mut self,
        receiver_id: SettlementId,
        offers: HashMap<ResourceType, TradeOffer>,
        active_usage: CaravanUsage,
    ) -> Result<(), TradeCommandError> {
        if receiver_id == self.id {
            return Err(TradeCommandError::TradeWithSelf);
        }
        if offers.is_empty() {
            return Err(TradeCommandError::EmptyOrder);
        }
        self.validate_offers(&offers)?;
        let (goods, gold) = TradeBook::funding_demand(&offers);
        if goods.is_empty() && gold == 0 {
            return Err(TradeCommandError::NothingToCarry);
        }
        let mut usage = self.queued_caravan_usage(active_usage);
        usage.add(offer_units(&offers));
        if self
            .trade_limits()
            .is_some_and(|limits| !usage.fits(limits))
        {
            return Err(TradeCommandError::CaravanCapacityExceeded);
        }
        let (market_goods, market_gold) = TradeBook::funding_demand(&self.fundable_market_offers());
        for (resource, &requested) in &goods {
            let available = self
                .inventory_manager
                .main
                .amount(resource.clone())
                .saturating_sub(market_goods.get(resource).copied().unwrap_or(0));
            if requested > available {
                return Err(TradeCommandError::InsufficientStock {
                    resource: resource.clone(),
                    requested: u64::from(requested),
                    available,
                });
            }
        }
        let available = self
            .gold()
            .saturating_sub(u32::try_from(market_gold).unwrap_or(u32::MAX));
        if gold > u64::from(available) {
            return Err(TradeCommandError::InsufficientGold {
                requested: gold,
                available,
            });
        }
        let gold = u32::try_from(gold).expect("budget cannot exceed the treasury");
        assert!(
            self.gold_inventory.consume(gold),
            "gold funding was validated"
        );
        for (resource, amount) in goods {
            assert!(
                self.inventory_manager.main.consume(resource, amount),
                "goods funding was validated"
            );
        }
        self.trade.orders.push(CaravanOrder::funded(
            receiver_id,
            Self::without_progress(offers),
            gold,
        ));
        Ok(())
    }

    /// Releases canceled escrow immediately when possible, retaining anything that cannot fit.
    pub fn cancel_caravan_orders(&mut self) {
        for order in std::mem::take(&mut self.trade.orders) {
            self.refund_canceled_order(CaravanReturn::undelivered(
                order.into_cargo(self.id),
                CaravanOutcome::OrderCanceled,
            ));
        }
    }

    /// Takes funded caravans ready to receive a route.
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
        if let Some((resource, _)) = offers
            .iter()
            .filter(|(_, offer)| offer.remaining_units() == 0)
            .min_by_key(|(resource, _)| *resource)
        {
            return Err(TradeCommandError::ZeroUnits(resource.clone()));
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
