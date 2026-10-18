//! Project validation, rejection and completion effects on Settlement.
use super::Settlement;
use super::building_manager::BuildingManager;
use super::projects::{ProjectCompletion, ProjectError, ProjectOutcome, ProjectResult};
use crate::domain::buildings::Building;
use crate::domain::world_data::map::Map;
use crate::services::algorithms::path_finder::PathFinder;

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

    pub(super) fn reject_invalid_projects(&mut self) -> Vec<ProjectOutcome> {
        let mut outcomes = Vec::new();
        let buildings = &self.building_manager;
        self.project_manager
            .active_projects
            .retain(
                |project| match Self::validate_project_result(&project.result, buildings) {
                    Ok(()) => true,
                    Err(error) => {
                        outcomes.push(ProjectOutcome {
                            project_id: project.id,
                            result: Err(error),
                        });
                        false
                    }
                },
            );
        outcomes
    }

    pub(super) fn resolve_completed_projects(
        &mut self,
        map: &Map,
        finder: &mut PathFinder,
    ) -> Vec<ProjectOutcome> {
        let completed = self.project_manager.extract_completed();
        let mut outcomes = Vec::new();
        for project in completed {
            let result = self.finalize_project(project.result, map, finder);
            outcomes.push(ProjectOutcome {
                project_id: project.id,
                result,
            });
        }
        outcomes
    }

    pub(super) fn finalize_project(
        &mut self,
        result: ProjectResult,
        map: &Map,
        finder: &mut PathFinder,
    ) -> Result<ProjectCompletion, ProjectError> {
        Self::validate_project_result(&result, &self.building_manager)?;
        match result {
            ProjectResult::ConstructBuilding {
                position,
                definition,
            } => {
                let id = self.building_manager.next_available_id();
                let new_building = Building::new(id, position, definition)
                    .map_err(ProjectError::InvalidDefinition)?;
                self.add_building(new_building, map, finder)
                    .map_err(ProjectError::DuplicateBuilding)?;
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
}
