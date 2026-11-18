//! One nation's turn, written for the nation agent. Later systems (taxes,
//! decrees, diplomacy) add their parts here.
use super::Nation;
use crate::domain::economy::ResourceType;
use crate::domain::ids::SettlementId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NationTurnReport {
    /// The rate in force during the turn (after this turn's decisions).
    pub tax_rate: f32,
    /// State at the end of the turn.
    pub capital: SettlementId,
    pub capital_loss_turns: u32,
    pub settlements: Vec<SettlementId>,
    pub closing_treasury: u32,
    pub closing_stock: BTreeMap<ResourceType, u32>,
}

impl NationTurnReport {
    /// The end-of-turn state of a nation owning `settlements`.
    pub(crate) fn closing(nation: &Nation, settlements: Vec<SettlementId>) -> Self {
        Self {
            tax_rate: nation.tax_rate(),
            capital: nation.capital(),
            capital_loss_turns: nation.capital_loss_turns(),
            settlements,
            closing_treasury: nation.treasury(),
            closing_stock: nation.stock_amounts(),
        }
    }
}
