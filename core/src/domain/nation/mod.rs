//! Nations (gameplay specification §5): state agents that own settlements,
//! hold a treasury and a national stock, and set policy. They produce and
//! trade nothing themselves.
//!
//! Which nation owns which settlement is recorded once, in
//! [`registry::NationRegistry`]. Rules that need settlements as well (capital
//! succession, extinction, subsidies) are coordinated by World.
pub mod decree;
pub mod decree_catalog;
pub mod registry;
pub mod report;
pub mod taxation;

use crate::domain::economy::gold_inventory::GoldInventory;
use crate::domain::economy::{InventoryAddResult, ResourceInventory, ResourceType};
use crate::domain::ids::{NationId, SettlementId};
use decree::DecreeBook;
use std::collections::BTreeMap;

/// One nation. Its settlements are not stored here: ownership lives in the
/// registry, and the capital is always one of the owned settlements.
///
/// Changing methods are `pub(crate)`: outside the crate a nation can only be
/// read, and every change goes through World, which keeps the invariants
/// (the capital is owned by this nation, the stock capacity follows ownership).
#[derive(Debug)]
pub struct Nation {
    id: NationId,
    capital: SettlementId,
    treasury: GoldInventory,
    /// One warehouse for all materials, like a settlement's: a single capacity
    /// of `STOCK_CAPACITY_PER_SETTLEMENT` × owned settlements, shared by every
    /// material. Gold is kept apart, in the treasury.
    stock: ResourceInventory,
    /// Common tax rate of all own settlements, a free fraction in 0.0..=1.0.
    tax_rate: f32,
    /// Turns left of the penalty for losing the capital (§4.2); its effect on
    /// happiness arrives with the happiness model.
    capital_loss_turns: u32,
    /// Pending, active and cooling-down decrees (§7).
    decrees: DecreeBook,
}

impl Nation {
    /// Capacity of the national stock contributed by every owned settlement,
    /// for all materials together.
    pub const STOCK_CAPACITY_PER_SETTLEMENT: u32 = 500;
    /// How long the penalty for losing the capital lasts.
    pub const CAPITAL_LOSS_PENALTY_TURNS: u32 = 10;
    /// The rate every nation starts with: 10%.
    pub const BASE_TAX_RATE: f32 = 0.1;

    /// A tax rate is any finite fraction from 0.0 (0%) to 1.0 (100%).
    pub fn is_valid_tax_rate(rate: f32) -> bool {
        rate.is_finite() && (0.0..=1.0).contains(&rate)
    }

    pub(crate) fn new(id: NationId, capital: SettlementId) -> Self {
        Self {
            id,
            capital,
            treasury: GoldInventory::default(),
            stock: ResourceInventory::new(Self::STOCK_CAPACITY_PER_SETTLEMENT),
            tax_rate: Self::BASE_TAX_RATE,
            capital_loss_turns: 0,
            decrees: DecreeBook::default(),
        }
    }

    pub fn id(&self) -> NationId {
        self.id
    }

    pub fn capital(&self) -> SettlementId {
        self.capital
    }

    pub fn treasury(&self) -> u64 {
        self.treasury.amount()
    }

    /// The national stock: amounts, the shared capacity and the stored total.
    pub fn stock(&self) -> &ResourceInventory {
        &self.stock
    }

    /// Stored materials with a non-zero amount, ascending by resource.
    pub fn stock_amounts(&self) -> BTreeMap<ResourceType, u32> {
        self.stock
            .resources()
            .iter()
            .filter(|&(resource, &amount)| resource != &ResourceType::Labour && amount > 0)
            .map(|(resource, &amount)| (resource.clone(), amount))
            .collect()
    }

    // ECONOMY (happiness, §4.2): the tax factor of happiness is
    // `10 − tax_rate × 100` points in every settlement of the nation.
    pub fn tax_rate(&self) -> f32 {
        self.tax_rate
    }

    /// Remaining turns of the capital-loss penalty; zero when none applies.
    // ECONOMY (happiness, §4.2): −10 happiness in every settlement of the
    // nation while this is above zero.
    pub fn capital_loss_turns(&self) -> u32 {
        self.capital_loss_turns
    }

    /// The caller checks `is_valid_tax_rate` (the action validator does).
    pub(crate) fn set_tax_rate(&mut self, rate: f32) {
        debug_assert!(Self::is_valid_tax_rate(rate));
        self.tax_rate = rate;
    }

    pub(crate) fn deposit_gold(&mut self, amount: u64) {
        self.treasury.add(amount);
    }

    /// Takes exactly `amount`, or nothing when the treasury holds less.
    pub(crate) fn withdraw_gold(&mut self, amount: u64) -> bool {
        self.treasury.consume(amount)
    }

    /// Empties the treasury and returns what it held.
    pub(crate) fn take_treasury(&mut self) -> u64 {
        self.treasury.consume_available()
    }

    /// Adds materials up to the shared capacity; the rest is rejected (lost to
    /// the caller). Labour is a per-turn capacity of a population and is never
    /// stored.
    pub(crate) fn deposit_resource(
        &mut self,
        resource: ResourceType,
        amount: u32,
    ) -> InventoryAddResult {
        if resource == ResourceType::Labour {
            return InventoryAddResult {
                accepted: 0,
                rejected: amount,
            };
        }
        self.stock.add(resource, amount)
    }

    /// Takes exactly `amount`, or nothing when less is stored. Zero always succeeds.
    pub(crate) fn withdraw_resource(&mut self, resource: ResourceType, amount: u32) -> bool {
        amount == 0 || self.stock.consume(resource, amount)
    }

    /// Empties the stock and returns what it held.
    pub(crate) fn take_stock(&mut self) -> BTreeMap<ResourceType, u32> {
        self.stock
            .take_all()
            .into_iter()
            .filter(|(resource, amount)| *resource != ResourceType::Labour && *amount > 0)
            .collect()
    }

    /// Follows the number of owned settlements. A smaller capacity discards
    /// nothing; the stock above it simply accepts no more until it drops below.
    pub(crate) fn set_stock_capacity(&mut self, capacity: u32) {
        self.stock.resize(capacity);
    }

    pub fn decrees(&self) -> &DecreeBook {
        &self.decrees
    }

    pub(crate) fn decrees_mut(&mut self) -> &mut DecreeBook {
        &mut self.decrees
    }

    /// Moves the capital after the old one was lost and starts the penalty.
    pub(crate) fn replace_lost_capital(&mut self, capital: SettlementId) {
        self.capital = capital;
        self.capital_loss_turns = Self::CAPITAL_LOSS_PENALTY_TURNS;
    }

    /// Counts down per-turn timers at the end of a global turn.
    pub(crate) fn advance_turn(&mut self) {
        self.capital_loss_turns = self.capital_loss_turns.saturating_sub(1);
        self.decrees.advance_turn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stock_has_one_capacity_shared_by_all_materials() {
        let mut nation = Nation::new(NationId(1), SettlementId(1));
        nation.set_stock_capacity(10);
        assert_eq!(
            nation
                .deposit_resource(crate::test_support::material("wood"), 6)
                .accepted,
            6
        );
        let grain = nation.deposit_resource(crate::test_support::material("grain"), 6);
        assert_eq!(
            (grain.accepted, grain.rejected),
            (4, 2),
            "the capacity is shared"
        );
        assert_eq!(nation.deposit_resource(ResourceType::Labour, 4).rejected, 4);
        assert_eq!(nation.stock().total_amount(), 10);
        assert_eq!(
            nation.stock_amounts(),
            BTreeMap::from([
                (crate::test_support::material("wood"), 6),
                (crate::test_support::material("grain"), 4)
            ])
        );
    }

    #[test]
    fn a_smaller_capacity_discards_nothing_but_blocks_additions() {
        let mut nation = Nation::new(NationId(1), SettlementId(1));
        nation.set_stock_capacity(10);
        nation.deposit_resource(crate::test_support::material("wood"), 10);
        nation.set_stock_capacity(4);
        assert_eq!(
            nation.stock().amount(crate::test_support::material("wood")),
            10
        );
        assert_eq!(
            nation
                .deposit_resource(crate::test_support::material("grain"), 1)
                .rejected,
            1
        );
        assert!(nation.withdraw_resource(crate::test_support::material("wood"), 7));
        assert_eq!(
            nation
                .deposit_resource(crate::test_support::material("grain"), 5)
                .accepted,
            1
        );
        assert!(!nation.withdraw_resource(crate::test_support::material("wood"), 4));
        assert!(nation.withdraw_resource(crate::test_support::material("grain"), 0));
        assert_eq!(
            nation.take_stock(),
            BTreeMap::from([
                (crate::test_support::material("wood"), 3),
                (crate::test_support::material("grain"), 1)
            ])
        );
        assert_eq!(nation.stock().total_amount(), 0);
    }

    #[test]
    fn tax_rates_are_free_fractions_from_zero_to_one() {
        for valid in [0.0, 0.1, 0.123_456, 1.0] {
            assert!(Nation::is_valid_tax_rate(valid), "{valid}");
        }
        for invalid in [-0.01, 1.000_1, f32::NAN, f32::INFINITY] {
            assert!(!Nation::is_valid_tax_rate(invalid), "{invalid}");
        }
        let nation = Nation::new(NationId(1), SettlementId(1));
        assert_eq!(nation.tax_rate(), 0.1);
    }

    #[test]
    fn capital_loss_penalty_counts_down_to_zero() {
        let mut nation = Nation::new(NationId(1), SettlementId(1));
        nation.replace_lost_capital(SettlementId(2));
        assert_eq!(nation.capital(), SettlementId(2));
        for remaining in (0..Nation::CAPITAL_LOSS_PENALTY_TURNS).rev() {
            nation.advance_turn();
            assert_eq!(nation.capital_loss_turns(), remaining);
        }
        nation.advance_turn();
        assert_eq!(nation.capital_loss_turns(), 0);
    }

    #[test]
    fn treasury_keeps_gold_above_the_old_counter_limit() {
        let mut nation = Nation::new(NationId(1), SettlementId(1));
        let old_limit = u64::from(u32::MAX);
        nation.deposit_gold(old_limit);
        nation.deposit_gold(5);
        assert_eq!(nation.treasury(), old_limit + 5);
        assert!(nation.withdraw_gold(old_limit));
        assert!(!nation.withdraw_gold(6));
        assert_eq!(nation.take_treasury(), 5);
        assert_eq!(nation.treasury(), 0);
    }
}
