//! What one nation agent may see: its own treasury, stock, policy and a summary
//! of each own settlement. Other nations appear only by ID; their settlements
//! are visible to settlement agents as trade partners with their owner.
use super::observation::ResourceAmounts;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::TaxRate;
use crate::domain::nation::decree::{CapitalRelocation, Decree, DecreeEffects, DecreeState};
use crate::domain::nation::diplomacy::{Proposal, Relation};
use crate::domain::nation::taxation::NationTurnReport;
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
    pub tax_rate: TaxRate,
    pub treasury: u32,
    /// National stock, ascending resource; zero amounts are omitted.
    pub stock: ResourceAmounts,
    /// Capacity of the national stock for every single material.
    pub stock_capacity_per_resource: u32,
    /// Own settlements, ascending ID.
    pub settlements: Vec<OwnedSettlementObservation>,
    /// Other nations still in the game, ascending ID.
    pub other_nations: Vec<NationId>,
    /// Decrees, their slots and costs, and a pending capital relocation.
    pub decrees: DecreesObservation,
    /// Relations, proposals and the public reputation of all nations.
    pub diplomacy: DiplomacyObservation,
    /// Taxes collected in the latest completed turn: per settlement status,
    /// rate and production tax, what reached the stock and what was lost.
    /// None before the first turn.
    pub last_turn: Option<NationTurnReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedSettlementObservation {
    pub settlement_id: SettlementId,
    pub province_id: usize,
    pub population: u32,
    pub gold: u32,
    /// Freshly conquered: pays half the rate (`conquest_penalty` > 0).
    pub conquest_penalty: u32,
    /// An enemy army stands in the province; the settlement pays no tax.
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
    /// Current upkeep per turn of every decree, for the current number of settlements.
    pub upkeep: BTreeMap<Decree, u64>,
    /// Current cost of a capital relocation.
    pub relocation_cost: u64,
    /// A paid relocation that resolves at the start of the next turn.
    pub pending_relocation: Option<CapitalRelocation>,
    /// Combined effects of the active decrees.
    pub effects: DecreeEffects,
}

/// What a nation agent needs for diplomacy (§8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiplomacyObservation {
    /// The relation with every other nation.
    pub relations: BTreeMap<NationId, Relation>,
    /// Nations with which a treaty was broken this turn: no war until the next turn.
    pub warning_period: Vec<NationId>,
    /// Proposals to this nation; `answerable` ones can be answered now.
    pub incoming_proposals: Vec<Proposal>,
    /// Proposals of this nation still waiting for an answer.
    pub outgoing_proposals: Vec<Proposal>,
    /// Broken treaties of every nation, including this one (public, §8.4).
    pub reputation: BTreeMap<NationId, u32>,
}

impl NationObservation {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("observation values always serialize")
    }
}
