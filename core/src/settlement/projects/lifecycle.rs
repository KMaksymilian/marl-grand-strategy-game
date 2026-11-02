//! Project validation, rejection and completion effects on Settlement.
use super::{ProjectCompletion, ProjectError, ProjectOutcome, ProjectResult};
use crate::buildings::Building;
use crate::buildings::productivity::TravelCostSource;
use crate::ids::ProjectId;
use crate::settlement::Settlement;
use crate::settlement::economy::BuildingManager;
use crate::spatial::{BuildingReference, ConstructionSpace, ProjectReference};

impl Settlement {
    pub(in crate::settlement) fn validate_project_result(
        result: &ProjectResult,
        buildings: &BuildingManager,
    ) -> Result<(), ProjectError> {
        match result {
            ProjectResult::ConstructBuilding { .. } => Ok(()),
            ProjectResult::UpgradeBuilding {
                target_instance_id, ..
            } => {
                if buildings.contains(*target_instance_id) {
                    Ok(())
                } else {
                    Err(ProjectError::MissingBuilding(*target_instance_id))
                }
            }
            ProjectResult::RecruitUnit { .. } => Err(ProjectError::UnsupportedRecruitment),
        }
    }

    pub(in crate::settlement) fn reject_invalid_projects(
        &mut self,
        space: &mut impl ConstructionSpace,
    ) -> Vec<ProjectOutcome> {
        let mut outcomes = Vec::new();
        let buildings = &self.building_manager;
        let settlement_id = self.id;
        self.project_manager
            .active_projects
            .retain(|project| {
                match Self::validate_project_result(&project.result, buildings).and_then(|()| {
                    Self::validate_project_site(
                        ProjectReference {
                            settlement_id,
                            project_id: project.id,
                        },
                        &project.result,
                        space,
                    )
                }) {
                    Ok(()) => true,
                    Err(error) => {
                        Self::release_project_site(
                            ProjectReference {
                                settlement_id,
                                project_id: project.id,
                            },
                            &project.result,
                            space,
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

    pub(in crate::settlement) fn resolve_completed_projects(
        &mut self,
        environment: &mut (impl ConstructionSpace + TravelCostSource),
    ) -> Vec<ProjectOutcome> {
        let completed = self.project_manager.extract_completed();
        let mut outcomes = Vec::new();
        for project in completed {
            let result = self.finalize_project(project.id, project.result.clone(), environment);
            if result.is_err() {
                Self::release_project_site(
                    ProjectReference {
                        settlement_id: self.id,
                        project_id: project.id,
                    },
                    &project.result,
                    environment,
                );
            }
            outcomes.push(ProjectOutcome {
                project_id: project.id,
                result,
            });
        }
        outcomes
    }

    pub(in crate::settlement) fn finalize_project(
        &mut self,
        project_id: ProjectId,
        result: ProjectResult,
        environment: &mut (impl ConstructionSpace + TravelCostSource),
    ) -> Result<ProjectCompletion, ProjectError> {
        Self::validate_project_result(&result, &self.building_manager)?;
        Self::validate_project_site(
            ProjectReference {
                settlement_id: self.id,
                project_id,
            },
            &result,
            environment,
        )?;
        match result {
            ProjectResult::ConstructBuilding {
                position,
                definition,
            } => {
                let id = self.building_manager.next_available_id();
                let new_building = Building::new(id, position, definition);
                environment
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
                    .construct_building(new_building, environment)
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
        reference: ProjectReference,
        result: &ProjectResult,
        space: &impl ConstructionSpace,
    ) -> Result<(), ProjectError> {
        if let ProjectResult::ConstructBuilding { position, .. } = result {
            space
                .validate_construction(*position, reference)
                .map_err(ProjectError::Territory)?;
        }
        Ok(())
    }

    fn release_project_site(
        reference: ProjectReference,
        result: &ProjectResult,
        space: &mut impl ConstructionSpace,
    ) {
        if let ProjectResult::ConstructBuilding { position, .. } = result {
            space.release_rejected_construction(*position, reference);
        }
    }
}
