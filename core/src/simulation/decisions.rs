use std::sync::Arc;

use super::Simulation;
use crate::buildings::definition::BuildingDefinition;
use crate::economy::{ResourceType, storage_order::StorageOrderError};
use crate::ids::{BuildingId, ProjectId};
use crate::settlement::projects::ProjectCost;
use crate::world::{Position, WorldError};

/// Explicit caller-supplied commands; this does not implement an AI policy.
#[derive(Debug, Clone)]
pub enum SettlementDecision {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionError {
    World(WorldError),
    StorageOrder(StorageOrderError),
}

#[derive(Debug, Clone)]
pub struct DecisionOutcome {
    pub settlement_id: u32,
    pub decision: SettlementDecision,
    pub result: Result<(), DecisionError>,
}

impl Simulation {
    pub(super) fn apply_decision(
        &mut self,
        settlement_id: u32,
        decision: &SettlementDecision,
    ) -> Result<(), DecisionError> {
        match decision {
            SettlementDecision::Construct {
                project_id,
                position,
                definition,
                cost,
            } => self
                .world
                .queue_construction(
                    settlement_id,
                    *project_id,
                    *position,
                    definition.clone(),
                    cost.clone(),
                )
                .map_err(DecisionError::World),
            SettlementDecision::Upgrade {
                project_id,
                building_id,
                definition,
                cost,
            } => self
                .world
                .queue_upgrade(
                    settlement_id,
                    *project_id,
                    *building_id,
                    definition.clone(),
                    cost.clone(),
                )
                .map_err(DecisionError::World),
            SettlementDecision::CancelProject(project_id) => self
                .world
                .cancel_project(settlement_id, *project_id)
                .map_err(DecisionError::World),
            _ => {
                let (settlement, _, _) =
                    self.world
                        .settlement_turn_parts(settlement_id)
                        .ok_or(DecisionError::World(WorldError::MissingSettlement(
                            settlement_id,
                        )))?;
                match decision {
                    SettlementDecision::SetBuildingLoad { building_id, load } => settlement
                        .set_building_load(*building_id, *load)
                        .map_err(|error| DecisionError::World(WorldError::Building(error))),
                    SettlementDecision::SetBuildingActive {
                        building_id,
                        active,
                    } => settlement
                        .set_building_active(*building_id, *active)
                        .map_err(|error| DecisionError::World(WorldError::Building(error))),
                    SettlementDecision::SetStorageOrder(materials) => settlement
                        .set_storage_order(materials)
                        .map_err(DecisionError::StorageOrder),
                    _ => unreachable!("spatial commands are dispatched above"),
                }
            }
        }
    }
}
