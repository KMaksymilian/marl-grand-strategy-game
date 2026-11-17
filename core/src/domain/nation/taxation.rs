//! Taxation (gameplay specification §6): the reports written when the owning
//! nation takes its rate from each settlement's production.
//!
//! Only the production tax is collected. The gold tax needs the settlement's
//! gold income (§4.4), which depends on settlement happiness (§4.2); neither
//! exists in the settlement economy yet, so no gold is taxed until they do.
use super::TaxRate;
use crate::domain::economy::ResourceType;
use crate::domain::ids::{NationId, SettlementId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type MaterialTotals = BTreeMap<ResourceType, u64>;

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

/// Taxes of one settlement in one turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementTaxReport {
    pub nation_id: NationId,
    pub status: TaxStatus,
    /// The nation's rate.
    pub nominal_rate: TaxRate,
    /// The rate actually applied: nominal, half or zero (see `status`).
    pub effective_rate: TaxRate,
    /// Materials taken from this turn's gross production, per resource and
    /// rounded down; every produced material is taxed, including intermediate
    /// goods (grain and the bread made from earlier grain both count).
    pub production_tax: MaterialTotals,
}

/// What one nation collected in one turn, written for the nation agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NationTurnReport {
    pub tax_rate: TaxRate,
    /// Production taxes of all own settlements stored in the national stock.
    pub stock_received: MaterialTotals,
    /// Production taxes above the per-material capacity; they are lost.
    pub stock_lost: MaterialTotals,
    /// Every own settlement's taxes, ascending ID.
    pub settlements: BTreeMap<SettlementId, SettlementTaxReport>,
    pub closing_treasury: u32,
    pub closing_stock: BTreeMap<ResourceType, u32>,
}

impl NationTurnReport {
    pub(crate) fn new(tax_rate: TaxRate) -> Self {
        Self {
            tax_rate,
            stock_received: MaterialTotals::new(),
            stock_lost: MaterialTotals::new(),
            settlements: BTreeMap::new(),
            closing_treasury: 0,
            closing_stock: BTreeMap::new(),
        }
    }
}
