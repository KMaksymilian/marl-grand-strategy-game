//! What the military part of a turn did to one nation, for its turn report.
use super::UnitTypeId;
use crate::domain::ids::{ArmyId, NationId, SettlementId};
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

/// How an army's march of this turn ended (§11.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarchOutcome {
    /// Still on the way; it continues next turn.
    Marching,
    Arrived,
    /// Entered an enemy province and stopped there: it attacks its settlement.
    Attacking,
    /// No allowed route to the destination now; the order ended.
    NoRoute,
}

/// One army's march this turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmyMarch {
    pub army_id: ArmyId,
    /// Provinces entered this turn, in order.
    pub path: Vec<usize>,
    pub outcome: MarchOutcome,
}

/// An army that stood where it may no longer be (after a peace or a broken
/// alliance) and returned to the nearest own province; `to` is None when
/// there was none and the army was disbanded without its soldiers returning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmyWithdrawal {
    pub army_id: ArmyId,
    pub from: usize,
    pub to: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleSide {
    Attackers,
    Defenders,
}

/// One battle for a settlement (§11.3), reported to every nation involved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleReport {
    pub province_id: usize,
    pub settlement_id: SettlementId,
    /// The settlement's owner.
    pub defender: NationId,
    /// Nations of the attacking armies, ascending ID.
    pub attackers: Vec<NationId>,
    pub attacking_armies: Vec<ArmyId>,
    /// The owner's and its allies' armies in the province.
    pub defending_armies: Vec<ArmyId>,
    /// Garrison soldiers before the battle.
    pub garrison_soldiers: u32,
    /// Multiplier of the defence from walls, after siege engines (≥ 1).
    pub wall_multiplier: f32,
    /// Strength with the random variance applied.
    pub attacker_strength: f32,
    pub defender_strength: f32,
    /// A tie goes to the defenders.
    pub winner: BattleSide,
    /// Soldiers lost by each side (the garrison's count for the defenders
    /// and leave the population).
    pub attacker_losses: u32,
    pub defender_losses: u32,
    /// Attacking armies that retreated to the province they came from.
    pub retreated: Vec<ArmyId>,
    /// Armies destroyed: attackers without a way back, defenders of a fallen
    /// settlement, and armies without soldiers left.
    pub destroyed: Vec<ArmyId>,
    /// The settlement fell to this nation (§11.5).
    pub conquered_by: Option<NationId>,
}

/// The military part of a nation's turn report.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MilitaryTurnReport {
    pub upkeep: ArmyUpkeepReport,
    /// Armies returned home because they may no longer stand where they are.
    pub withdrawals: Vec<ArmyWithdrawal>,
    /// Marches of own armies this turn.
    pub marches: Vec<ArmyMarch>,
    /// Battles this nation took part in, as attacker, defender or ally.
    pub battles: Vec<BattleReport>,
    /// Recruitments finished at the end of this turn.
    pub recruits_joined: Vec<RecruitsJoined>,
}
