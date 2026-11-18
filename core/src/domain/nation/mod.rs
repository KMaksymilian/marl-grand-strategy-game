//! Nations (gameplay specification §5): state agents that own settlements,
//! hold a treasury and a national stock, and set policy. They produce and
//! trade nothing themselves.
//!
//! Which nation owns which settlement is recorded once, in
//! [`registry::NationRegistry`]. Rules that need settlements as well (capital
//! succession, extinction, subsidies) are coordinated by World.
pub mod registry;
pub mod report;
mod tax;

pub use tax::{InvalidTaxRate, TaxRate};

use crate::domain::economy::gold_inventory::GoldInventory;
use crate::domain::economy::{InventoryAddResult, ResourceType};
use crate::domain::ids::{NationId, SettlementId};
use std::collections::BTreeMap;

/// National material stock. Every material has its own capacity, set by the
/// number of owned settlements; additions above it are lost. Labour is never
/// stored.
///
/// Deliberately not `ResourceInventory`: a settlement warehouse has one capacity
/// shared by all materials, while the national stock has a separate limit per
/// material. The capacity is passed in by the caller because it depends on
/// ownership, which only the registry knows.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NationalStock {
    resources: BTreeMap<ResourceType, u32>,
}

impl NationalStock {
    pub fn amount(&self, resource: ResourceType) -> u32 {
        self.resources.get(&resource).copied().unwrap_or(0)
    }

    /// Stored materials with a non-zero amount, ascending by resource.
    pub fn resources(&self) -> impl Iterator<Item = (ResourceType, u32)> + '_ {
        self.resources
            .iter()
            .filter(|(_, amount)| **amount > 0)
            .map(|(&resource, &amount)| (resource, amount))
    }

    /// Accepts up to `capacity` units of this material in total; the rest is
    /// reported as rejected (lost to the caller).
    pub(crate) fn add(
        &mut self,
        resource: ResourceType,
        amount: u32,
        capacity: u32,
    ) -> InventoryAddResult {
        if resource == ResourceType::Labour {
            return InventoryAddResult {
                accepted: 0,
                rejected: amount,
            };
        }
        let stored = self.amount(resource);
        let accepted = amount.min(capacity.saturating_sub(stored));
        if accepted > 0 {
            self.resources.insert(resource, stored + accepted);
        }
        InventoryAddResult {
            accepted,
            rejected: amount - accepted,
        }
    }

    /// Removes exactly `amount`, or nothing when less is stored.
    pub(crate) fn take(&mut self, resource: ResourceType, amount: u32) -> bool {
        let stored = self.amount(resource);
        if stored < amount {
            return false;
        }
        self.resources.insert(resource, stored - amount);
        true
    }

    /// Cuts every material down to `capacity` and returns what was discarded.
    pub(crate) fn enforce_capacity(&mut self, capacity: u32) -> BTreeMap<ResourceType, u32> {
        let mut lost = BTreeMap::new();
        for (&resource, amount) in &mut self.resources {
            if *amount > capacity {
                lost.insert(resource, *amount - capacity);
                *amount = capacity;
            }
        }
        lost
    }

    /// Empties the stock and returns what it held.
    pub(crate) fn take_all(&mut self) -> BTreeMap<ResourceType, u32> {
        std::mem::take(&mut self.resources)
            .into_iter()
            .filter(|(_, amount)| *amount > 0)
            .collect()
    }
}

/// One nation. Its settlements are not stored here: ownership lives in the
/// registry, and the capital is always one of the owned settlements.
///
/// Changing methods are `pub(crate)`: outside the crate a nation can only be
/// read, and every change goes through World, which keeps the invariants
/// (the capital is owned by this nation, the stock is within capacity).
#[derive(Debug)]
pub struct Nation {
    id: NationId,
    capital: SettlementId,
    treasury: GoldInventory,
    stock: NationalStock,
    tax_rate: TaxRate,
    /// Turns left of the penalty for losing the capital (§4.2); its effect on
    /// happiness arrives with the happiness model.
    capital_loss_turns: u32,
}

impl Nation {
    /// Per-material stock capacity contributed by every owned settlement.
    pub const STOCK_CAPACITY_PER_SETTLEMENT: u32 = 500;
    /// How long the penalty for losing the capital lasts.
    pub const CAPITAL_LOSS_PENALTY_TURNS: u32 = 10;

    pub(crate) fn new(id: NationId, capital: SettlementId) -> Self {
        Self {
            id,
            capital,
            treasury: GoldInventory::default(),
            stock: NationalStock::default(),
            tax_rate: TaxRate::BASE,
            capital_loss_turns: 0,
        }
    }

    pub fn id(&self) -> NationId {
        self.id
    }

    pub fn capital(&self) -> SettlementId {
        self.capital
    }

    pub fn treasury(&self) -> u32 {
        self.treasury.amount()
    }

    pub fn stock(&self) -> &NationalStock {
        &self.stock
    }

    pub fn tax_rate(&self) -> TaxRate {
        self.tax_rate
    }

    /// Remaining turns of the capital-loss penalty; zero when none applies.
    pub fn capital_loss_turns(&self) -> u32 {
        self.capital_loss_turns
    }

    pub(crate) fn set_tax_rate(&mut self, rate: TaxRate) {
        self.tax_rate = rate;
    }

    /// Adds gold up to the counter limit and returns the accepted amount.
    /// `GoldInventory::add` would overflow on huge sums, so the amount is capped first.
    pub(crate) fn deposit_gold(&mut self, amount: u32) -> u32 {
        let accepted = amount.min(u32::MAX - self.treasury.amount());
        self.treasury.add(accepted);
        accepted
    }

    /// Takes exactly `amount`, or nothing when the treasury holds less.
    pub(crate) fn withdraw_gold(&mut self, amount: u32) -> bool {
        self.treasury.consume(amount)
    }

    /// Empties the treasury and returns what it held.
    pub(crate) fn take_treasury(&mut self) -> u32 {
        self.treasury.consume_available()
    }

    pub(crate) fn stock_mut(&mut self) -> &mut NationalStock {
        &mut self.stock
    }

    /// Moves the capital after the old one was lost and starts the penalty.
    pub(crate) fn replace_lost_capital(&mut self, capital: SettlementId) {
        self.capital = capital;
        self.capital_loss_turns = Self::CAPITAL_LOSS_PENALTY_TURNS;
    }

    /// Counts down per-turn timers at the end of a global turn.
    pub(crate) fn advance_turn(&mut self) {
        self.capital_loss_turns = self.capital_loss_turns.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ResourceType::{Grain, Labour, Wood};

    #[test]
    fn stock_capacity_is_per_material_and_overflow_is_reported() {
        let mut stock = NationalStock::default();
        assert_eq!(
            stock.add(Wood, 8, 10),
            InventoryAddResult {
                accepted: 8,
                rejected: 0
            }
        );
        assert_eq!(
            stock.add(Wood, 5, 10),
            InventoryAddResult {
                accepted: 2,
                rejected: 3
            }
        );
        // Another material has its own capacity.
        assert_eq!(stock.add(Grain, 10, 10).accepted, 10);
        assert_eq!(stock.add(Labour, 4, 10).rejected, 4);
        assert_eq!(stock.amount(Labour), 0);
        assert_eq!(
            stock.resources().collect::<Vec<_>>(),
            vec![(Wood, 10), (Grain, 10)]
        );
    }

    #[test]
    fn take_is_all_or_nothing_and_capacity_cuts_report_losses() {
        let mut stock = NationalStock::default();
        stock.add(Wood, 10, 10);
        stock.add(Grain, 3, 10);
        assert!(!stock.take(Wood, 11));
        assert!(stock.take(Wood, 4));
        assert_eq!(stock.amount(Wood), 6);
        assert_eq!(stock.enforce_capacity(5), BTreeMap::from([(Wood, 1)]));
        assert_eq!((stock.amount(Wood), stock.amount(Grain)), (5, 3));
        assert_eq!(stock.take_all(), BTreeMap::from([(Wood, 5), (Grain, 3)]));
        assert_eq!(stock.resources().count(), 0);
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
    fn treasury_never_overflows() {
        let mut nation = Nation::new(NationId(1), SettlementId(1));
        assert_eq!(nation.deposit_gold(u32::MAX - 1), u32::MAX - 1);
        assert_eq!(nation.deposit_gold(5), 1);
        assert_eq!(nation.treasury(), u32::MAX);
        assert!(nation.withdraw_gold(u32::MAX - 1));
        assert!(!nation.withdraw_gold(2));
        assert_eq!(nation.take_treasury(), 1);
        assert_eq!(nation.treasury(), 0);
    }
}
