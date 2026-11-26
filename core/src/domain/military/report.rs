//! What the military part of a turn did to one nation, for its turn report.
use super::UnitTypeId;
use crate::domain::ids::{ArmyId, SettlementId};
use serde::{Deserialize, Serialize};

/// Army upkeep of one nation in one turn (§10.3). Every unit receives the
/// same share of what was available: the morale of all units changes alike.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmyUpkeepReport {
    /// Living soldiers in the nation's armies when the upkeep was due.
    pub soldiers: u32,
    pub food_demand: u64,
    /// Food taken from the national stock, tier-2 food first.
    pub food_paid: u64,
    /// The part of `food_paid` that was tier-2 food.
    pub tier2_food_paid: u64,
    pub gold_demand: u64,
    pub gold_paid: u64,
    /// Food and gold were both paid in full.
    pub fully_supplied: bool,
    /// Morale change of every unit this turn.
    pub morale_change: i32,
    /// Units whose morale reached zero: they deserted and are gone.
    pub deserted: Vec<DesertedUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesertedUnit {
    pub army_id: ArmyId,
    pub unit_type: UnitTypeId,
    pub soldiers: u32,
}

/// Recruited units that became ready this turn and joined an army.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecruitsJoined {
    pub army_id: ArmyId,
    pub settlement_id: SettlementId,
    pub unit_type: UnitTypeId,
    pub units: u32,
}

/// The military part of a nation's turn report.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MilitaryTurnReport {
    pub upkeep: ArmyUpkeepReport,
    /// Recruitments finished at the end of this turn.
    pub recruits_joined: Vec<RecruitsJoined>,
}
