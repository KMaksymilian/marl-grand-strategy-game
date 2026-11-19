//! One nation's turn, written for the nation agent. Later systems (diplomacy)
//! add their parts here.
use super::Nation;
use super::decree::{Decree, DecreeState, DecreeUpkeepReport, RelocationOutcome};
use super::diplomacy::DiplomaticEvent;
use super::taxation::NationTaxReport;
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
    /// What the nation's settlements paid this turn (§6).
    pub taxes: NationTaxReport,
    /// Decrees at the end of the turn: pending, active or cooling down (§7).
    pub decrees: BTreeMap<Decree, DecreeState>,
    /// Decree upkeep paid this turn, and decrees revoked because the treasury
    /// could not pay them.
    pub decree_upkeep: DecreeUpkeepReport,
    /// A relocation ordered last turn, resolved at the start of this one.
    pub capital_relocation: Option<RelocationOutcome>,
    /// Diplomatic events concerning this nation this turn, in order: wars
    /// declared on it, allies joining, proposals received and answered,
    /// broken treaties, embargoes, expired proposals, ended truces (§8).
    pub diplomacy: Vec<DiplomaticEvent>,
}

impl NationTurnReport {
    /// The end-of-turn state of a nation owning `settlements`, with this
    /// turn's taxes.
    pub(crate) fn closing(
        nation: &Nation,
        settlements: Vec<SettlementId>,
        taxes: NationTaxReport,
    ) -> Self {
        Self {
            tax_rate: nation.tax_rate(),
            capital: nation.capital(),
            capital_loss_turns: nation.capital_loss_turns(),
            settlements,
            closing_treasury: nation.treasury(),
            closing_stock: nation.stock_amounts(),
            taxes,
            decrees: nation.decrees().states().clone(),
            decree_upkeep: DecreeUpkeepReport::default(),
            capital_relocation: None,
            diplomacy: Vec::new(),
        }
    }
}
