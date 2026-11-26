//! What one nation agent may see: its own treasury, stock, policy and a summary
//! of each own settlement. Other nations appear only by ID; settlement agents
//! see the owner of each trade partner.
use super::observation::ResourceAmounts;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::decree::{DecreeEffects, DecreeId, DecreeState};
use crate::domain::nation::diplomacy::{Proposal, Relation};
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
    pub treasury: u64,
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
    /// Decree slots, states, upkeep and effects (§7).
    pub decrees: DecreesObservation,
    /// Relations, own proposals and the public reputation of all nations (§8).
    pub diplomacy: DiplomacyObservation,
    /// The nation's report of the latest completed turn; None before the first.
    pub last_turn: Option<NationTurnReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedSettlementObservation {
    pub settlement_id: SettlementId,
    pub province_id: usize,
    pub population: u32,
    pub gold: u64,
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
    /// Active turns before a decree can be revoked, and the cooldown after.
    pub minimum_active_turns: u32,
    pub cooldown_turns: u32,
    /// Every decree that is pending, active or cooling down; others are free.
    pub states: BTreeMap<DecreeId, DecreeState>,
    /// Upkeep per active turn of every decree of the catalogue, for the
    /// current number of settlements. Its keys are every decree that exists.
    pub upkeep: BTreeMap<DecreeId, u64>,
    /// Combined effects of the active decrees.
    pub effects: DecreeEffects,
}

/// What a nation agent needs for diplomacy (§8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiplomacyObservation {
    /// The relation with every other nation.
    pub relations: BTreeMap<NationId, RelationObservation>,
    /// Nations with which a treaty was broken this turn: no war until the next turn.
    pub warning_period: Vec<NationId>,
    /// Proposals to this nation; `answerable` ones can be answered now.
    pub incoming_proposals: Vec<Proposal>,
    /// This nation's proposals still waiting for an answer.
    pub outgoing_proposals: Vec<Proposal>,
    /// Broken treaties of every nation, this one included (public, §8.4).
    pub reputation: BTreeMap<NationId, u32>,
}

/// The relation with one other nation and how long it has held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationObservation {
    pub relation: Relation,
    /// The global turn in which this relation started; 0 when it has held
    /// since the start of the game. With `turn` it gives e.g. the length of a
    /// war or the age of an alliance.
    pub since_turn: u64,
}

impl NationObservation {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("observation values always serialize")
    }
}
