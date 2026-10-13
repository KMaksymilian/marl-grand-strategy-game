use super::Settlement;
use crate::domain::buildings::core::{Building, BuildingLocation, Position};
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::economy::resource::ResourceType;
use crate::domain::economy::storage_order::StorageOrderError;
use crate::domain::ids::{BuildingId, ProjectId};
use crate::domain::settlement::projects::{Project, ProjectCost, ProjectError, ProjectResult};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingCommandError {
    MissingBuilding(BuildingId),
    InvalidEfficiency,
}

// Commands and immutable views belong to Settlement itself.
// This module adds no wrapper, state copy or lifetime parameter.
impl Settlement {
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn population(&self) -> u32 {
        self.population_manager.population
    }
    pub fn inventory(&self) -> &crate::domain::economy::resource::ResourceInventory {
        &self.inventory_manager.main
    }
    pub fn pending_inventory(&self) -> &crate::domain::economy::resource::ResourceInventory {
        &self.inventory_manager.pending_production
    }
    pub fn deposit(
        &mut self,
        resource: ResourceType,
        amount: u32,
    ) -> crate::domain::economy::resource::InventoryAddResult {
        self.inventory_manager.main.add(resource, amount)
    }
    pub fn withdraw(&mut self, resource: ResourceType, amount: u32) -> bool {
        self.inventory_manager.main.consume(resource, amount)
    }
    pub fn set_inventory_capacity(
        &mut self,
        capacity: u32,
    ) -> Result<(), crate::domain::economy::resource::CapacityError> {
        self.inventory_manager.main.set_capacity(capacity)
    }
    pub fn buildings(&self) -> impl Iterator<Item = &Building> {
        self.building_manager
            .hub_buildings
            .iter()
            .chain(&self.building_manager.spoke_buildings)
    }
    pub fn active_projects(&self) -> &[Project] {
        &self.project_manager.active_projects
    }
    pub fn add_building(&mut self, building: Building) -> Result<(), BuildingId> {
        self.building_manager.construct_building(building)
    }
    pub fn set_building_efficiency(
        &mut self,
        id: BuildingId,
        value: f32,
    ) -> Result<(), BuildingCommandError> {
        self.building_manager
            .hub_buildings
            .iter_mut()
            .chain(&mut self.building_manager.spoke_buildings)
            .find(|building| building.instance_id() == id)
            .ok_or(BuildingCommandError::MissingBuilding(id))?
            .set_target_efficiency(value)
            .map_err(|_| BuildingCommandError::InvalidEfficiency)
    }
    pub fn set_building_active(
        &mut self,
        id: BuildingId,
        active: bool,
    ) -> Result<(), BuildingCommandError> {
        self.building_manager
            .hub_buildings
            .iter_mut()
            .chain(&mut self.building_manager.spoke_buildings)
            .find(|building| building.instance_id() == id)
            .ok_or(BuildingCommandError::MissingBuilding(id))?
            .set_active(active);
        Ok(())
    }

    /// Applies to incoming storage batches; does not evict existing stock.
    pub fn set_storage_order(
        &mut self,
        materials: impl AsRef<[ResourceType]>,
    ) -> Result<(), StorageOrderError> {
        self.inventory_manager.set_storage_order(materials)
    }

    pub fn storage_order(&self) -> &[ResourceType] {
        self.inventory_manager.storage_order()
    }

    // Request new building
    pub fn queue_construction(
        &mut self,
        project_id: ProjectId,
        location: BuildingLocation,
        position: Position,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) -> Result<(), ProjectError> {
        let project = Project::new(
            project_id,
            ProjectResult::ConstructBuilding {
                location,
                position,
                definition,
            },
            cost,
        );

        Self::validate_project_result(&project.result, &self.building_manager)?;
        self.project_manager
            .add_project(project)
            .map_err(ProjectError::DuplicateProject)
    }

    // Request upgrading existing building
    pub fn queue_upgrade(
        &mut self,
        project_id: ProjectId,
        target_instance_id: BuildingId,
        new_definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) -> Result<(), ProjectError> {
        let result = ProjectResult::UpgradeBuilding {
            target_instance_id,
            new_definition,
        };
        Self::validate_project_result(&result, &self.building_manager)?;
        let project = Project::new(project_id, result, cost);

        Self::validate_project_result(&project.result, &self.building_manager)?;
        self.project_manager
            .add_project(project)
            .map_err(ProjectError::DuplicateProject)
    }
}
