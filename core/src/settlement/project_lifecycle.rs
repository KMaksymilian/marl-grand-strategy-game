//! Project validation, rejection and completion effects on Settlement.
use super::Settlement;
use super::building_manager::BuildingManager;
use super::projects::{ProjectCompletion, ProjectError, ProjectOutcome, ProjectResult};
use crate::buildings::Building;
use crate::ids::ProjectId;
use crate::ids::SettlementId;
use crate::world::map::Map;
use crate::world::pathfinding::path_finder::PathFinder;
use crate::world::territory::{
    BuildingReference, ProjectReference, TerritoryManager, TileOccupancy,
};

impl Settlement {
    pub(super) fn validate_project_result(
        result: &ProjectResult,
        buildings: &BuildingManager,
    ) -> Result<(), ProjectError> {
        match result {
            ProjectResult::ConstructBuilding { definition, .. } => definition
                .validate()
                .map_err(ProjectError::InvalidDefinition),
            ProjectResult::UpgradeBuilding {
                target_instance_id,
                new_definition,
            } => {
                new_definition
                    .validate()
                    .map_err(ProjectError::InvalidDefinition)?;
                if buildings.contains(*target_instance_id) {
                    Ok(())
                } else {
                    Err(ProjectError::MissingBuilding(*target_instance_id))
                }
            }
            ProjectResult::RecruitUnit { .. } => Err(ProjectError::UnsupportedRecruitment),
        }
    }

    pub(super) fn reject_invalid_projects(
        &mut self,
        map: &Map,
        territory: &mut TerritoryManager,
    ) -> Vec<ProjectOutcome> {
        let mut outcomes = Vec::new();
        let buildings = &self.building_manager;
        let settlement_id = self.id;
        self.project_manager
            .active_projects
            .retain(|project| {
                match Self::validate_project_result(&project.result, buildings).and_then(|()| {
                    Self::validate_project_site(
                        settlement_id,
                        project.id,
                        &project.result,
                        map,
                        territory,
                    )
                }) {
                    Ok(()) => true,
                    Err(error) => {
                        Self::release_project_site(
                            settlement_id,
                            project.id,
                            &project.result,
                            territory,
                        );
                        outcomes.push(ProjectOutcome {
                            project_id: project.id,
                            result: Err(error),
                        });
                        false
                    }
                }
            });
        outcomes
    }

    pub(super) fn resolve_completed_projects(
        &mut self,
        map: &Map,
        finder: &mut PathFinder,
        territory: &mut TerritoryManager,
    ) -> Vec<ProjectOutcome> {
        let completed = self.project_manager.extract_completed();
        let mut outcomes = Vec::new();
        for project in completed {
            let result =
                self.finalize_project(project.id, project.result.clone(), map, finder, territory);
            if result.is_err() {
                Self::release_project_site(self.id, project.id, &project.result, territory);
            }
            outcomes.push(ProjectOutcome {
                project_id: project.id,
                result,
            });
        }
        outcomes
    }

    pub(super) fn finalize_project(
        &mut self,
        project_id: ProjectId,
        result: ProjectResult,
        map: &Map,
        finder: &mut PathFinder,
        territory: &mut TerritoryManager,
    ) -> Result<ProjectCompletion, ProjectError> {
        Self::validate_project_result(&result, &self.building_manager)?;
        Self::validate_project_site(self.id, project_id, &result, map, territory)?;
        match result {
            ProjectResult::ConstructBuilding {
                position,
                definition,
            } => {
                let id = self.building_manager.next_available_id();
                let new_building = Building::new(id, position, definition)
                    .map_err(ProjectError::InvalidDefinition)?;
                territory
                    .finish_construction(
                        position,
                        ProjectReference {
                            settlement_id: self.id,
                            project_id,
                        },
                        BuildingReference {
                            settlement_id: self.id,
                            building_id: id,
                        },
                    )
                    .map_err(ProjectError::Territory)?;
                self.building_manager
                    .construct_building(new_building, map, finder)
                    .expect("new building ID is unused");
                Ok(ProjectCompletion::BuildingConstructed(id))
            }
            ProjectResult::UpgradeBuilding {
                target_instance_id,
                new_definition,
            } => {
                if self
                    .building_manager
                    .upgrade_building(target_instance_id, new_definition)
                {
                    Ok(ProjectCompletion::BuildingUpgraded(target_instance_id))
                } else {
                    Err(ProjectError::MissingBuilding(target_instance_id))
                }
            }
            ProjectResult::RecruitUnit { .. } => Err(ProjectError::UnsupportedRecruitment),
        }
    }

    fn validate_project_site(
        settlement_id: SettlementId,
        project_id: ProjectId,
        result: &ProjectResult,
        map: &Map,
        territory: &TerritoryManager,
    ) -> Result<(), ProjectError> {
        if let ProjectResult::ConstructBuilding { position, .. } = result {
            map.validate_building_position(*position)
                .map_err(ProjectError::Territory)?;
            territory
                .validate_reservation(
                    *position,
                    ProjectReference {
                        settlement_id,
                        project_id,
                    },
                )
                .map_err(ProjectError::Territory)?;
        }
        Ok(())
    }

    fn release_project_site(
        settlement_id: SettlementId,
        project_id: ProjectId,
        result: &ProjectResult,
        territory: &mut TerritoryManager,
    ) {
        if let ProjectResult::ConstructBuilding { position, .. } = result {
            let reference = ProjectReference {
                settlement_id,
                project_id,
            };
            if territory.tile_at(*position).occupancy() == TileOccupancy::Construction(reference) {
                territory
                    .cancel_construction(*position, reference)
                    .expect("reservation belongs to the rejected project");
            }
        }
    }
}
