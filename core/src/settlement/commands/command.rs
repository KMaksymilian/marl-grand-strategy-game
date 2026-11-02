//! Command values independent of the simulation scheduler and map implementation.
use crate::buildings::definition::BuildingDefinition;
use crate::economy::{ResourceType, cost::ProjectCost};
use crate::ids::{BuildingId, ProjectId};
use crate::spatial::Position;
use std::sync::Arc;

/// Explicit caller-supplied commands; this does not implement an AI policy.
#[derive(Debug, Clone)]
pub enum SettlementCommand {
    Construct {
        project_id: ProjectId,
        position: Position,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    },
    Upgrade {
        project_id: ProjectId,
        building_id: BuildingId,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    },
    CancelProject(ProjectId),
    SetBuildingLoad {
        building_id: BuildingId,
        load: f32,
    },
    SetBuildingActive {
        building_id: BuildingId,
        active: bool,
    },
    SetStorageOrder(Vec<ResourceType>),
}
