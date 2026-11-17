//! Nations: state agents that own settlements, hold a treasury and a national
//! stock, and set policy. Ownership itself lives in [`registry::NationRegistry`],
//! the only record of which nation owns which settlement.
pub mod registry;

use crate::domain::economy::gold_inventory::GoldInventory;
use crate::domain::economy::{InventoryAddResult, ResourceType};
use crate::domain::ids::{NationId, SettlementId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The common tax rate in basis points (1 = 0.01%), from 0 to 10 000 (100%).
/// Serializes as the plain integer; values above 10 000 do not parse.
///
/// The specification asks for a continuous rate. An integer in basis points is
/// fine-grained enough for that, stays deterministic (no float rounding between
/// Rust and Python) and cannot be NaN. The private field plus `try_from` in serde
/// mean an invalid rate can never exist in memory, so later code need not re-check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct TaxRate(u16);

/// A tax rate above 100%.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTaxRate(pub u16);

impl fmt::Display for InvalidTaxRate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "tax rate {} exceeds {} basis points",
            self.0,
            TaxRate::MAX_BASIS_POINTS
        )
    }
}

impl TaxRate {
    pub const MAX_BASIS_POINTS: u16 = 10_000;
    /// 10%: the rate every nation starts with and the neutral rate for happiness.
    pub const BASE: Self = Self(1_000);
    pub const ZERO: Self = Self(0);
    pub const FULL: Self = Self(Self::MAX_BASIS_POINTS);

    pub fn from_basis_points(basis_points: u16) -> Result<Self, InvalidTaxRate> {
        if basis_points > Self::MAX_BASIS_POINTS {
            return Err(InvalidTaxRate(basis_points));
        }
        Ok(Self(basis_points))
    }

    pub fn basis_points(self) -> u16 {
        self.0
    }
}

impl Default for TaxRate {
    fn default() -> Self {
        Self::BASE
    }
}

impl TryFrom<u16> for TaxRate {
    type Error = InvalidTaxRate;

    fn try_from(basis_points: u16) -> Result<Self, Self::Error> {
        Self::from_basis_points(basis_points)
    }
}

impl From<TaxRate> for u16 {
    fn from(rate: TaxRate) -> Self {
        rate.0
    }
}

/// National material stock. Each material has its own capacity, set by the
/// number of owned settlements; additions above it are lost. Labour is never stored.
///
/// This is deliberately not `ResourceInventory`: a settlement warehouse has one
/// capacity shared by all materials, while the specification gives the national
/// stock a separate limit per material (500 per owned settlement). The capacity
/// is passed in by the caller because it depends on ownership, which only the
/// registry knows.
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

    pub(crate) fn take_all(&mut self) -> BTreeMap<ResourceType, u32> {
        std::mem::take(&mut self.resources)
            .into_iter()
            .filter(|(_, amount)| *amount > 0)
            .collect()
    }
}

/// One nation. Its settlements are not stored here: ownership is recorded once,
/// in the registry, and the capital is always one of the owned settlements.
///
/// Mutating methods are `pub(crate)`: outside the crate a nation can only be
/// read, and every change goes through World, which keeps the invariants
/// (capital owned by this nation, stock within capacity).
#[derive(Debug)]
pub struct Nation {
    id: NationId,
    capital: SettlementId,
    treasury: GoldInventory,
    stock: NationalStock,
    tax_rate: TaxRate,
    /// Turns left of the penalty for losing the capital (gameplay §4.2).
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

    pub(crate) fn withdraw_gold(&mut self, amount: u32) -> bool {
        self.treasury.consume(amount)
    }

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
    fn tax_rate_accepts_zero_to_one_hundred_percent() {
        assert_eq!(TaxRate::from_basis_points(0), Ok(TaxRate::ZERO));
        assert_eq!(TaxRate::from_basis_points(10_000), Ok(TaxRate::FULL));
        assert_eq!(
            TaxRate::from_basis_points(10_001),
            Err(InvalidTaxRate(10_001))
        );
        assert_eq!(TaxRate::default().basis_points(), 1_000);
    }

    #[test]
    fn tax_rate_serializes_as_basis_points_and_rejects_values_above_full() {
        assert_eq!(serde_json::to_string(&TaxRate::BASE).unwrap(), "1000");
        assert_eq!(
            serde_json::from_str::<TaxRate>("2550").unwrap(),
            TaxRate::from_basis_points(2_550).unwrap()
        );
        assert!(serde_json::from_str::<TaxRate>("10001").is_err());
        assert!(serde_json::from_str::<TaxRate>("-1").is_err());
        assert!(serde_json::from_str::<TaxRate>("12.5").is_err());
    }

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
        assert_eq!(stock.amount(Wood), 5);
        assert_eq!(stock.amount(Grain), 3);
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
