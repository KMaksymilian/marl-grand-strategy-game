//! Construction and upgrade requests with explicit costs and typed IDs.
use super::super::Settlement;
use super::super::projects::{Project, ProjectCost, ProjectError, ProjectResult};
use crate::domain::buildings::definition::BuildingDefinition;
use crate::domain::ids::{BuildingId, ProjectId};
use crate::domain::spatial::Position;
use crate::domain::spatial::{ConstructionSpace, ProjectReference};
use std::sync::Arc;

impl Settlement {
    /// Stops future spending. Already paid materials remain consumed.
    /// Failed cancellation preserves both the project and the spatial registry.
    pub fn cancel_project(
        &mut self,
        project_id: ProjectId,
        space: &mut impl ConstructionSpace,
    ) -> Result<(), ProjectError> {
        let index = self
            .project_manager
            .active_projects
            .iter()
            .position(|project| project.id() == project_id)
            .ok_or(ProjectError::MissingProject(project_id))?;
        let project = &self.project_manager.active_projects[index];
        if let ProjectResult::ConstructBuilding { position, .. } = project.result() {
            space
                .cancel_construction(
                    *position,
                    ProjectReference {
                        settlement_id: self.id(),
                        project_id,
                    },
                )
                .map_err(ProjectError::Territory)?;
        }
        // Releasing construction was the only fallible mutation. Upgrades retain
        // their building and do not release its structural slot.
        self.project_manager.active_projects.remove(index);
        Ok(())
    }

    /// Requests construction and reserves its position in the shared registry.
    pub fn queue_construction(
        &mut self,
        project_id: ProjectId,
        position: Position,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
        space: &mut impl ConstructionSpace,
    ) -> Result<(), ProjectError> {
        self.validate_project_materials(&definition, &cost)?;
        self.validate_new_building_rules(&definition)
            .map_err(ProjectError::BuildingRule)?;
        space
            .evaluate_site(position, definition.site_requirements())
            .map_err(ProjectError::Territory)?;
        let project = Project::new(
            project_id,
            ProjectResult::ConstructBuilding {
                position,
                definition,
            },
            cost,
        );

        if self
            .active_projects()
            .iter()
            .any(|project| project.id() == project_id)
        {
            return Err(ProjectError::DuplicateProject(project_id));
        }
        let reference = ProjectReference {
            settlement_id: self.id(),
            project_id,
        };
        space
            .reserve_construction(position, reference)
            .map_err(ProjectError::Territory)?;
        // The project ID was checked before reserving. Keep both stores consistent.
        self.project_manager.add_project(project).map_err(|id| {
            space
                .cancel_construction(position, reference)
                .expect("new reservation is unchanged");
            ProjectError::DuplicateProject(id)
        })
    }

    // Request upgrading existing building
    pub fn queue_upgrade(
        &mut self,
        project_id: ProjectId,
        target_instance_id: BuildingId,
        new_definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) -> Result<(), ProjectError> {
        self.validate_project_materials(&new_definition, &cost)?;
        let building = self
            .building_manager
            .get(target_instance_id)
            .ok_or(ProjectError::MissingBuilding(target_instance_id))?;
        new_definition
            .site_requirements()
            .evaluate(building.position(), |_| None)
            .map_err(ProjectError::Territory)?;
        self.queue_validated_upgrade(project_id, target_instance_id, new_definition, cost)
    }

    /// Requests an upgrade whose location requirements can be checked by the environment.
    pub fn queue_upgrade_with_space(
        &mut self,
        project_id: ProjectId,
        target_instance_id: BuildingId,
        new_definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
        space: &impl ConstructionSpace,
    ) -> Result<(), ProjectError> {
        self.validate_project_materials(&new_definition, &cost)?;
        let building = self
            .building_manager
            .get(target_instance_id)
            .ok_or(ProjectError::MissingBuilding(target_instance_id))?;
        space
            .evaluate_site(building.position(), new_definition.site_requirements())
            .map_err(ProjectError::Territory)?;
        self.queue_validated_upgrade(project_id, target_instance_id, new_definition, cost)
    }

    fn queue_validated_upgrade(
        &mut self,
        project_id: ProjectId,
        target_instance_id: BuildingId,
        new_definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) -> Result<(), ProjectError> {
        self.validate_building_upgrade_rules(target_instance_id, &new_definition)?;
        let result = ProjectResult::UpgradeBuilding {
            target_instance_id,
            new_definition,
        };
        let project = Project::new(project_id, result, cost);

        self.project_manager
            .add_project(project)
            .map_err(ProjectError::DuplicateProject)
    }

    fn validate_project_materials(
        &self,
        definition: &BuildingDefinition,
        cost: &ProjectCost,
    ) -> Result<(), ProjectError> {
        let unknown = self
            .first_unknown_building_material(definition)
            .or_else(|| {
                cost.needed_resources
                    .iter()
                    .filter(|(resource, amount)| **amount > 0 && !self.knows_material(resource))
                    .map(|(resource, _)| resource)
                    .min()
                    .cloned()
            });
        match unknown {
            Some(resource) => Err(ProjectError::UnknownMaterial(resource)),
            None => Ok(()),
        }
    }
}
