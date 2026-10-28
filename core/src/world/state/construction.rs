//! World entry points validate province membership; settlements own project lifecycle.
use std::sync::Arc;

use super::{World, WorldError};
use crate::buildings::{Building, definition::BuildingDefinition};
use crate::ids::{BuildingId, ProjectId};
use crate::settlement::projects::{ProjectCost, ProjectError, ProjectResult};
use crate::settlement::turn_report::TurnReport;
use crate::world::{Position, pathfinding::path_finder::PathFinder};

impl World {
    pub fn queue_construction(
        &mut self,
        settlement_id: u32,
        project_id: ProjectId,
        position: Position,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) -> Result<(), WorldError> {
        self.validate_settlement_position(settlement_id, position)?;
        self.settlements
            .get_mut(&settlement_id)
            .expect("settlement was validated")
            .queue_construction(
                project_id,
                position,
                definition,
                cost,
                &self.map,
                &mut self.territory,
            )
            .map_err(WorldError::Project)
    }

    pub fn add_building(
        &mut self,
        settlement_id: u32,
        building: Building,
        finder: &mut PathFinder,
    ) -> Result<(), WorldError> {
        self.validate_settlement_position(settlement_id, building.position())?;
        self.settlements
            .get_mut(&settlement_id)
            .expect("settlement was validated")
            .add_building(building, &self.map, finder, &mut self.territory)
            .map_err(WorldError::Building)
    }

    pub fn queue_upgrade(
        &mut self,
        settlement_id: u32,
        project_id: ProjectId,
        building_id: BuildingId,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) -> Result<(), WorldError> {
        let settlement = self
            .settlement(settlement_id)
            .ok_or(WorldError::MissingSettlement(settlement_id))?;
        let building = settlement
            .buildings()
            .find(|building| building.instance_id() == building_id)
            .ok_or(WorldError::Project(ProjectError::MissingBuilding(
                building_id,
            )))?;
        self.validate_settlement_position(settlement_id, building.position())?;
        self.settlements
            .get_mut(&settlement_id)
            .expect("settlement was validated")
            .queue_upgrade(project_id, building_id, definition, cost)
            .map_err(WorldError::Project)
    }

    /// Advances one settlement; this does not implement a global world turn scheduler.
    pub fn process_settlement_turn(
        &mut self,
        settlement_id: u32,
        finder: &mut PathFinder,
    ) -> Result<TurnReport, WorldError> {
        let settlement = self
            .settlement(settlement_id)
            .ok_or(WorldError::MissingSettlement(settlement_id))?;
        // Map layers remain mutable for world generation. Check membership again
        // before spending resources if those layers were edited between turns.
        for project in settlement.active_projects() {
            match project.result() {
                ProjectResult::ConstructBuilding { position, .. } => {
                    self.validate_settlement_position(settlement_id, *position)?
                }
                ProjectResult::UpgradeBuilding {
                    target_instance_id, ..
                } => {
                    if let Some(building) = settlement
                        .buildings()
                        .find(|building| building.instance_id() == *target_instance_id)
                    {
                        self.validate_settlement_position(settlement_id, building.position())?;
                    }
                }
                ProjectResult::RecruitUnit { .. } => {}
            }
        }
        Ok(self
            .settlements
            .get_mut(&settlement_id)
            .expect("settlement was validated")
            .process_turn(&self.map, finder, &mut self.territory))
    }
}
