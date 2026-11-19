//! What one nation agent may see: its own treasury, stock, policy and a summary
//! of each own settlement. Other nations appear only by ID; settlement agents
//! see the owner of each trade partner.
use super::observation::ResourceAmounts;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::decree::{CapitalRelocation, Decree, DecreeEffects, DecreeState};
use crate::domain::nation::report::NationTurnReport;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NationObservation {
    /// Completed global turns when the observation was generated.
    pub turn: u64,
    pub nation_id: NationId,
    pub capital: SettlementId,
    /// Remaining turns of the penalty for losing the capital; zero when none.
    pub capital_loss_turns: u32,
    pub tax_rate: f32,
    pub treasury: u32,
    /// National stock, ascending resource; zero amounts are omitted.
    pub stock: ResourceAmounts,
    /// Materials stored in total and the capacity they share. The stock may
    /// exceed the capacity after losing a settlement; then it accepts nothing.
    pub stock_total: u64,
    pub stock_capacity: u32,
    /// Own settlements, ascending ID.
    pub settlements: Vec<OwnedSettlementObservation>,
    /// Other nations still in the game, ascending ID.
    pub other_nations: Vec<NationId>,
    /// Decrees, slots, costs and a pending capital relocation.
    pub decrees: DecreesObservation,
    /// The nation's report of the latest completed turn; None before the first.
    pub last_turn: Option<NationTurnReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedSettlementObservation {
    pub settlement_id: SettlementId,
    pub province_id: usize,
    pub population: u32,
    pub gold: u32,
    /// Turns left of the conquest penalty: the settlement pays half the rate.
    pub conquest_turns: u32,
    /// An enemy army stands in the province: the settlement pays no tax.
    pub under_attack: bool,
    /// Free warehouse space, the limit for material subsidies.
    pub free_capacity: u64,
}

/// What a nation agent needs to choose decrees (§7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecreesObservation {
    pub slots: usize,
    /// Slots used by pending and active decrees.
    pub used_slots: usize,
    /// Every decree that is pending, active or cooling down; others are free.
    pub states: BTreeMap<Decree, DecreeState>,
    /// One-time cost of every decree now, for the current number of settlements.
    pub costs: BTreeMap<Decree, u64>,
    /// Cost of a capital relocation now.
    pub relocation_cost: u64,
    /// A paid relocation that resolves at the start of the next turn.
    pub pending_relocation: Option<CapitalRelocation>,
    /// Combined effects of the active decrees.
    pub effects: DecreeEffects,
}

impl NationObservation {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("observation values always serialize")
    }
}
