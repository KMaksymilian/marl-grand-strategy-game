//! Serializable results returned for commands after execution.
use super::contract::ActionKind;
use crate::domain::economy::{ResourceType, storage_order::StorageOrderError};
use crate::domain::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use crate::domain::spatial::Position;
use serde::{Deserialize, Serialize};

/// Why one action of a decision was rejected. A rejected action changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionError {
    UnknownBuildingType {
        building_type: BuildingTypeId,
    },
    /// No building with this ID belongs to the settlement.
    UnknownBuilding {
        building_id: BuildingId,
    },
    /// No active project with this ID belongs to the settlement.
    UnknownProject {
        project_id: ProjectId,
    },
    /// Caravan receiver is not a registered settlement.
    UnknownSettlement {
        settlement_id: SettlementId,
    },
    /// Outside the map.
    InvalidPosition {
        position: Position,
    },
    /// On the map but not in the settlement's own province.
    OutsideOwnProvince {
        position: Position,
    },
    OceanTile {
        position: Position,
    },
    RequiredDepositMissing {
        position: Position,
        resource: ResourceType,
    },
    /// A building or construction site already occupies the tile.
    TileOccupied {
        position: Position,
    },
    /// The building already has an active upgrade project.
    BuildingAlreadyUpgrading {
        building_id: BuildingId,
    },
    SameBuildingType {
        building_id: BuildingId,
    },
    WorkersAboveCapacity {
        building_id: BuildingId,
        workers: u32,
        max_workers: u32,
    },
    /// The requested staff of all buildings would exceed the population.
    InsufficientWorkers {
        requested: u64,
        available: u32,
    },
    /// Valid count that the building's load cannot express exactly.
    UnsupportedWorkerCount {
        building_id: BuildingId,
        workers: u32,
    },
    InvalidStorageOrder {
        error: StorageOrderError,
    },
    LabourIsNotTradable,
    ZeroUnits {
        resource: ResourceType,
    },
    DuplicateOffer {
        resource: ResourceType,
    },
    /// A caravan order without offers.
    EmptyOrder,
    /// A caravan order that would carry neither goods nor gold (e.g. only
    /// purchases at price 0); the engine would skip it.
    NothingToCarry,
    TradeWithSelf,
    /// Not enough stock for this action at funding time: `available` is what is
    /// left after the market (funded first) and earlier caravan orders.
    InsufficientStock {
        resource: ResourceType,
        requested: u64,
        available: u32,
    },
    /// Not enough gold for this action at funding time: `available` is what is
    /// left after the market (funded first) and earlier caravan orders.
    InsufficientGold {
        requested: u64,
        available: u32,
    },
    ProjectIdsExhausted,
    /// An earlier action of the same decision already targets the same object.
    Conflict {
        with_index: usize,
    },
    /// The engine refused an action that passed validation. This indicates a
    /// mismatch between validator and engine rules; the state is unchanged.
    ExecutionFailed,
}

/// What an applied action changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionEffect {
    /// Construction or upgrade queued under a simulation-assigned ID.
    ProjectQueued {
        project_id: ProjectId,
    },
    ProjectCanceled {
        project_id: ProjectId,
    },
    /// The building is active and requests this staff from the next allocation on.
    WorkersSet {
        building_id: BuildingId,
        workers: u32,
    },
    StorageOrderSet,
    MarketOffersPublished,
    MarketOffersCleared,
    /// Funded from stock and gold during this turn, then sent to the receiver.
    CaravanOrdered {
        receiver_id: SettlementId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionOutcome {
    Applied { effect: ActionEffect },
    Rejected { error: ActionError },
}

/// Result of one submitted action, identified by its position in the decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionResult {
    pub index: usize,
    pub kind: ActionKind,
    pub outcome: ActionOutcome,
}

impl ActionResult {
    pub fn is_applied(&self) -> bool {
        matches!(self.outcome, ActionOutcome::Applied { .. })
    }
}

/// Results of one decision, one entry per action in submission order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionOutcome {
    pub settlement_id: SettlementId,
    pub results: Vec<ActionResult>,
}

/// A rejected action, identified by its position in `SettlementDecision::actions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RejectedAction {
    pub index: usize,
    pub kind: ActionKind,
    pub error: ActionError,
}
