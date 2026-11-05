//! Which actions a settlement may submit now, part of its observation.
//!
//! Every option is checked by the action validator against the current state, as
//! the first action of a decision. Two kinds of rule depend on the rest of the
//! decision and are not expressed here; the validator still enforces them:
//! - at most one action per target (tile, building, project, storage order,
//!   market), see `ActionError::Conflict`;
//! - caravan stock and gold limits are shared by all caravans of one decision.
use crate::economy::ResourceType;
use crate::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use crate::settlement::decision::ActionKind;
use crate::spatial::Position;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionMask {
    /// Every `ActionKind`, in `ActionKind::ALL` order: whether at least one
    /// action of that kind is currently valid.
    pub available: BTreeMap<ActionKind, bool>,
    /// `Construct`: each buildable tile with the building types allowed there.
    pub construct: Vec<ConstructOption>,
    /// `Upgrade`: each building that can be upgraded with its allowed types.
    pub upgrade: Vec<UpgradeOption>,
    /// `CancelProject`: active projects.
    pub cancel_project: Vec<ProjectId>,
    /// `SetBuildingWorkers`: each building with its valid range `0..=max_workers`.
    pub set_building_workers: Vec<WorkerRange>,
    /// `SetStorageOrder`: a valid order lists each of these exactly once.
    pub storage_materials: Vec<ResourceType>,
    /// Resources allowed in market and caravan offers; at most one offer each,
    /// with at least one unit.
    pub tradable_resources: Vec<ResourceType>,
    /// `OrderCaravan`: valid receivers.
    pub caravan_receivers: Vec<SettlementId>,
    /// Units a caravan may sell per resource: the current stock.
    pub caravan_sell_limits: BTreeMap<ResourceType, u32>,
    /// Gold a caravan may commit to purchases (units × price): the treasury.
    pub caravan_gold_limit: u32,
}

impl ActionMask {
    pub fn is_available(&self, kind: ActionKind) -> bool {
        self.available.get(&kind).copied().unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructOption {
    pub position: Position,
    pub building_types: Vec<BuildingTypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeOption {
    pub building_id: BuildingId,
    pub building_types: Vec<BuildingTypeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerRange {
    pub building_id: BuildingId,
    /// Highest valid `workers` value; every value from 0 up to it is valid.
    pub max_workers: u32,
}
