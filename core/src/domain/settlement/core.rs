use crate::domain::buildings::core::{Building, BuildingLocation, Position};
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::economy::resource_allocation::{AllocationEngine, AllocationRecipient};
use crate::domain::ids::{BuildingId, ProjectId};
use crate::domain::settlement::building_manager::BuildingManager;
use crate::domain::settlement::demographics::{NeedRegistry, PopulationManager};
use crate::domain::settlement::project_manager::ProjectManager;
use crate::domain::settlement::projects::{
    Project, ProjectCompletion, ProjectCost, ProjectError, ProjectOutcome, ProjectResult,
};
use crate::domain::settlement::settlement_inventory_manager::SettlementInventoryManager;
use crate::domain::settlement::turn_report::{InventoryTurnReport, TurnReport};
use std::sync::Arc;

#[cfg(test)]
#[path = "turn_tests.rs"]
mod turn_tests;

#[derive(Debug)]
pub struct Settlement {
    pub id: u32,
    pub inventory_manager: SettlementInventoryManager,
    pub building_manager: BuildingManager,
    pub population_manager: PopulationManager,
    pub project_manager: ProjectManager,
}

impl Settlement {
    pub fn new(
        id: u32,
        initial_capacity: u32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
    ) -> Self {
        Self {
            id,
            inventory_manager: SettlementInventoryManager::new(initial_capacity),
            building_manager: BuildingManager::new(),
            population_manager: PopulationManager::new(initial_population, need_registry),
            project_manager: ProjectManager::new(),
        }
    }

    pub fn process_turn(&mut self) -> TurnReport {
        let opening_stock = InventoryTurnReport::snapshot(
            &self.inventory_manager.main,
            &self.inventory_manager.pending_production,
        );
        // Invalid projects must not participate in material allocation.
        let mut projects = self.reject_invalid_projects();
        // Labour update
        self.inventory_manager
            .refresh_labor_capacity(self.population_manager.population);

        // 1. Demand phase
        let mut requests = Vec::new();
        requests.extend(self.population_manager.generate_requests());
        requests.extend(self.project_manager.generate_requests());
        requests.extend(self.building_manager.generate_requests());

        // 2. Allocation phase
        let allocations = AllocationEngine::execute(&mut self.inventory_manager.main, requests);
        let mut inventory = InventoryTurnReport::from_allocations(opening_stock, &allocations);

        // 3. Consumption and production phase
        let consumption_result = self
            .population_manager
            .process_allocation(allocations.get(&AllocationRecipient::Population));
        self.project_manager.process_allocation(&allocations);
        let (produced, returned) = self.building_manager.process_allocations(&allocations);
        inventory.record_production(&produced, &returned);

        // 4. Collecting phase
        inventory.losses = self
            .inventory_manager
            .process_production_results(produced, returned);
        inventory
            .losses
            .extend(self.inventory_manager.commit_pending_production());
        inventory.closing_stock = InventoryTurnReport::snapshot(
            &self.inventory_manager.main,
            &self.inventory_manager.pending_production,
        );

        // 5. Project finalization phase
        projects.extend(self.resolve_completed_projects());

        TurnReport {
            consumption: consumption_result,
            projects,
            inventory,
        }
    }

    fn validate_project_result(
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

    fn reject_invalid_projects(&mut self) -> Vec<ProjectOutcome> {
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

    fn resolve_completed_projects(&mut self) -> Vec<ProjectOutcome> {
        let completed = self.project_manager.extract_completed();
        let mut outcomes = Vec::new();
        for project in completed {
            let result = self.finalize_project(project.result);
            outcomes.push(ProjectOutcome {
                project_id: project.id,
                result,
            });
        }
        outcomes
    }

    fn finalize_project(
        &mut self,
        result: ProjectResult,
    ) -> Result<ProjectCompletion, ProjectError> {
        Self::validate_project_result(&result, &self.building_manager)?;
        match result {
            ProjectResult::ConstructBuilding {
                location,
                position,
                definition,
            } => {
                let id = self.building_manager.next_available_id();
                let new_building = Building::new(id, position, location, definition);
                self.building_manager
                    .construct_building(new_building)
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

        self.project_manager
            .add_project(project)
            .map_err(ProjectError::DuplicateProject)
    }
}
