//! What one nation agent may see: its own treasury, stock, policy and a summary
//! of each own settlement. Other nations appear only by ID; their settlements
//! are visible to settlement agents as trade partners with their owner.
use super::observation::ResourceAmounts;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::TaxRate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedSettlementObservation {
    pub settlement_id: SettlementId,
    pub province_id: usize,
    pub population: u32,
    pub gold: u32,
    /// Free warehouse space, the limit for material subsidies.
    pub free_capacity: u64,
}

impl NationObservation {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("observation values always serialize")
    }
}
