//! Taxation (gameplay specification §6): the rate rule and the reports written
//! when the owning nation takes its share of each settlement's production.
//!
//! Only the production tax exists. The gold tax applies to a settlement's gold
//! income (§4.4), which depends on happiness (§4.2); both belong to the
//! settlement economy and are not implemented yet (see `ECONOMY` notes).
use crate::domain::economy::ResourceType;
use crate::domain::ids::{NationId, SettlementId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type MaterialTotals = BTreeMap<ResourceType, u64>;

/// The tax on `amount` whole units at `rate`, rounded down: a settlement never
/// pays a fraction of a unit and never more than it has; 100% takes everything.
///
/// The `f32` rate is first rounded to six decimals: an `f32` keeps about seven
/// significant digits, so 0.29 is stored as 0.28999999… and would tax 100
/// units as 28. Rounding restores the 0.29 the agent meant.
pub fn tax_amount(rate: f32, amount: u64) -> u64 {
    let rate = (f64::from(rate) * 1e6).round() / 1e6;
    let exact = amount as f64 * rate;
    // A relative tolerance keeps exact products (e.g. 0.1 × 100 = 10.000…002
    // or 9.999…98) on the whole number they represent.
    let rounded = (exact * (1.0 + 1e-12)).floor();
    (rounded.max(0.0) as u64).min(amount)
}

/// Which share of the nation's rate a settlement pays this turn (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxStatus {
    /// The full rate.
    Regular,
    /// Half the rate while the conquest penalty lasts.
    FreshlyConquered,
    /// No tax: an enemy army stands in the province.
    UnderAttack,
}

impl TaxStatus {
    /// The rate actually paid for the nation's `rate`.
    pub fn effective_rate(self, rate: f32) -> f32 {
        match self {
            Self::Regular => rate,
            Self::FreshlyConquered => rate / 2.0,
            Self::UnderAttack => 0.0,
        }
    }
}

/// Taxes of one settlement in one turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementTaxReport {
    pub nation_id: NationId,
    pub status: TaxStatus,
    /// The nation's rate.
    pub nominal_rate: f32,
    /// The rate actually applied: nominal, half or zero (see `status`).
    pub effective_rate: f32,
    /// Materials taken from this turn's gross production, per resource and
    /// rounded down. Every produced material is taxed, intermediate goods too
    /// (grain, and bread made from earlier grain, both count).
    pub production_tax: MaterialTotals,
    // ECONOMY (gold income, §4.4 and §6): add `gold_income` and `gold_tax`
    // here when settlements earn gold; the gold tax is `income × effective_rate`.
}

/// What a nation collected in one turn.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NationTaxReport {
    /// Production taxes of all own settlements stored in the national stock.
    pub stock_received: MaterialTotals,
    /// Production taxes that did not fit the stock's capacity; they are lost.
    pub stock_lost: MaterialTotals,
    /// Every own settlement's taxes, ascending ID.
    pub settlements: BTreeMap<SettlementId, SettlementTaxReport>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxes_round_down_to_whole_units_without_float_artifacts() {
        assert_eq!(tax_amount(0.1, 100), 10);
        assert_eq!(tax_amount(0.1, 99), 9, "9.9 rounds down");
        assert_eq!(tax_amount(0.1, 9), 0, "less than one unit is not taxed");
        assert_eq!(tax_amount(0.29, 100), 29, "not 28 from 0.28999999 in f32");
        assert_eq!(tax_amount(0.07, 100), 7);
        assert_eq!(tax_amount(0.57, 100), 57);
        assert_eq!(tax_amount(0.333_333, 3), 0);
        assert_eq!(tax_amount(0.0, u64::from(u32::MAX)), 0);
        assert_eq!(tax_amount(1.0, 12_345), 12_345);
        assert_eq!(tax_amount(1.0, u64::from(u32::MAX)), u64::from(u32::MAX));
    }

    #[test]
    fn status_sets_the_effective_rate() {
        assert_eq!(TaxStatus::Regular.effective_rate(0.2), 0.2);
        assert_eq!(TaxStatus::FreshlyConquered.effective_rate(0.2), 0.1);
        assert_eq!(TaxStatus::UnderAttack.effective_rate(0.2), 0.0);
    }
}
