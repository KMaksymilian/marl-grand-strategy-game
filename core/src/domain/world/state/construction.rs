//! World entry points validate province membership; settlements own project lifecycle.
use crate::domain::ids::SettlementId;
use std::sync::Arc;

use super::{World, WorldError};
use crate::domain::buildings::{Building, definition::BuildingDefinition};
use crate::domain::ids::{BuildingId, ProjectId};
use crate::domain::settlement::projects::{ProjectCost, ProjectError, ProjectResult};
#[cfg(test)]
use crate::domain::settlement::turn_report::TurnReport;
use crate::domain::world::settlement_environment::{
    MapConstructionSpace, MapSettlementEnvironment,
};
use crate::domain::world::{Position, pathfinding::path_finder::PathFinder};

impl World {
    /// Read-only placement check for previews and command preparation.
    pub(crate) fn validate_construction_site(
        &self,
        settlement_id: SettlementId,
        position: Position,
    ) -> Result<(), WorldError> {
        self.validate_settlement_position(settlement_id, position)?;
        self.map
            .validate_building_position(position)
            .and_then(|_| self.territory.validate_vacancy(position))
            .map_err(|error| WorldError::Project(ProjectError::Territory(error)))
    }

    pub(crate) fn validate_blueprint_site(
        &self,
        settlement_id: SettlementId,
        position: Position,
        definition: &BuildingDefinition,
    ) -> Result<(), WorldError> {
        self.validate_construction_site(settlement_id, position)?;
        self.validate_building_deposit(position, definition.required_deposit())
    }

    pub(crate) fn validate_upgrade_site(
        &self,
        settlement_id: SettlementId,
        position: Position,
        definition: &BuildingDefinition,
    ) -> Result<(), WorldError> {
        self.validate_settlement_position(settlement_id, position)?;
        self.validate_building_deposit(position, definition.required_deposit())
    }

    /// Cancellation belongs to the settlement and also works after map edits.
    pub fn cancel_project(
        &mut self,
        settlement_id: SettlementId,
        project_id: ProjectId,
    ) -> Result<(), WorldError> {
        self.settlements
            .get_mut(settlement_id)
            .ok_or(WorldError::MissingSettlement(settlement_id))?
            .cancel_project(
                project_id,
                &mut MapConstructionSpace::new(&self.map, &mut self.territory),
            )
            .map_err(WorldError::Project)
    }

    pub fn queue_construction(
        &mut self,
        settlement_id: SettlementId,
        project_id: ProjectId,
        position: Position,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) -> Result<(), WorldError> {
        self.validate_blueprint_site(settlement_id, position, &definition)?;
        self.settlements
            .get_mut(settlement_id)
            .expect("settlement was validated")
            .queue_construction(
                project_id,
                position,
                definition,
                cost,
                &mut MapConstructionSpace::new(&self.map, &mut self.territory),
            )
            .map_err(WorldError::Project)
    }

    pub fn add_building(
        &mut self,
        settlement_id: SettlementId,
        building: Building,
        finder: &mut PathFinder,
    ) -> Result<(), WorldError> {
        self.validate_settlement_position(settlement_id, building.position())?;
        self.validate_building_deposit(
            building.position(),
            building.definition().required_deposit(),
        )?;
        self.settlements
            .get_mut(settlement_id)
            .expect("settlement was validated")
            .add_building(
                building,
                &mut MapSettlementEnvironment::new(&self.map, finder, &mut self.territory),
            )
            .map_err(WorldError::Building)
    }

    pub fn queue_upgrade(
        &mut self,
        settlement_id: SettlementId,
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
        self.validate_upgrade_site(settlement_id, building.position(), &definition)?;
        self.settlements
            .get_mut(settlement_id)
            .expect("settlement was validated")
            .queue_upgrade(project_id, building_id, definition, cost)
            .map_err(WorldError::Project)
    }

    /// Advances one registered settlement in isolation for tests.
    #[cfg(test)]
    pub(crate) fn process_settlement_turn_for_test(
        &mut self,
        settlement_id: SettlementId,
        finder: &mut PathFinder,
    ) -> Result<TurnReport, WorldError> {
        self.validate_settlement_turn(settlement_id)?;
        let province = self
            .settlement_province(settlement_id)
            .expect("registered settlement");
        let mut visitors: Vec<_> = self
            .caravans
            .iter_mut()
            .filter(|caravan| {
                caravan.state() == crate::domain::trade::caravan::CaravanState::Trading
                    && caravan.receiver_id == settlement_id
            })
            .collect();
        visitors.sort_by_key(|caravan| caravan.sender_id);
        Ok(self
            .settlements
            .get_mut(settlement_id)
            .expect("settlement was validated")
            .process_turn_for_test(
                &mut MapSettlementEnvironment::new(&self.map, finder, &mut self.territory)
                    .with_province(province),
                &mut visitors,
            ))
    }

    pub(crate) fn validate_settlement_turn(
        &self,
        settlement_id: SettlementId,
    ) -> Result<(), WorldError> {
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
                    target_instance_id,
                    new_definition,
                } => {
                    if let Some(building) = settlement
                        .buildings()
                        .find(|building| building.instance_id() == *target_instance_id)
                    {
                        self.validate_upgrade_site(
                            settlement_id,
                            building.position(),
                            new_definition,
                        )?;
                    }
                }
                ProjectResult::RecruitUnit { .. } => {}
            }
        }
        Ok(())
    }
}
