use std::collections::HashMap;
use crate::domain::buildings::core::{BuildingLocation};
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::economy::resource_allocation::{AllocationEngine, AllocationResult, DemandRequest};
use crate::domain::settlement::demographics::{ConsumptionResult, NeedRegistry, PopulationManager};
use crate::domain::settlement::project_manager::ProjectManager;
use crate::domain::settlement::projects::{Project, ProjectCost, ProjectResult};
use crate::domain::settlement::settlement_inventory_manager::SettlementInventoryManager;
use std::sync::Arc;
use crate::domain::economy::resource::ResourceType;
use crate::domain::world_data::point::Point;


pub enum  SettlementCommand{
    ClaimTile {
        settlement_id: u32,
        target_position: Point
    },
    ConstructBuilding {
        settlement_id: u32,
        instance_id: u32,
        location: BuildingLocation,
        position: Point,
        definition: Arc<BuildingDefinition>,
    },
    UpgradeBuilding {
        settlement_id: u32,
        target_instance_id: u32,
        new_definition: Arc<BuildingDefinition>,
    }
}
#[derive(Debug)]
pub struct Settlement {
    pub id: u32,
    pub inventory_manager: SettlementInventoryManager,
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
            population_manager: PopulationManager::new(initial_population, need_registry),
            project_manager: ProjectManager::new(),
        }
    }


    /// Phase 2: Collecting Demands and allocating resources
    pub fn prepare_allocations(&mut self, building_requests: Vec<DemandRequest>) -> AllocationResult {
        self.inventory_manager.refresh_labor_capacity(self.population_manager.population);

        let mut requests = Vec::new();
        requests.extend(self.population_manager.generate_requests());
        requests.extend(self.project_manager.generate_requests());
        requests.extend(building_requests);

        AllocationEngine::execute(&mut self.inventory_manager.main, requests)
    }

    /// Phase 3 : Applying allocation to internal systems
    pub fn apply_internal_allocations(&mut self, allocations: &AllocationResult) -> ConsumptionResult {
        let consumption_result = self.population_manager.process_allocation(allocations.get(&0));
        self.project_manager.process_allocation(allocations);

        consumption_result
    }

    /// Phase 4 & 5: Collecting resources and project finalization
    pub fn finalize_turn(
        &mut self,
        buildings_produced: HashMap<ResourceType, u32>,
        buildings_returned: HashMap<ResourceType, u32>,
    ) -> Vec<SettlementCommand> {

        self.inventory_manager.process_production_results(buildings_produced, buildings_returned);
        self.inventory_manager.commit_pending_production();

        self.resolve_completed_projects()
    }

    fn resolve_completed_projects(&mut self) -> Vec<SettlementCommand> {
        let completed = self.project_manager.extract_completed();
        let mut commands = Vec::new();

        for project in completed {
            match project.result {
                ProjectResult::ConstructBuilding { location, position, definition } => {
                    commands.push(SettlementCommand::ConstructBuilding {
                        settlement_id: self.id,
                        instance_id: project.id,
                        location,
                        position,
                        definition,
                    });
                }
                ProjectResult::UpgradeBuilding { target_instance_id, new_definition } => {
                    commands.push(SettlementCommand::UpgradeBuilding {
                        settlement_id: self.id,
                        target_instance_id,
                        new_definition,
                    });
                }
                ProjectResult::ClaimTile { target_position } => {
                    commands.push(SettlementCommand::ClaimTile {
                        settlement_id: self.id,
                        target_position,
                    });
                }
                ProjectResult::RecruitUnit { .. } => {
                    // TODO
                }
            }
        }

        commands
    }

    // Request new building
    pub fn queue_construction(
        &mut self,
        project_id: u32,
        location: BuildingLocation,
        position: Point,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) {
        let project = Project::new(
            project_id,
            ProjectResult::ConstructBuilding {
                location,
                position,
                definition,
            },
            cost,
        );

        self.project_manager.add_project(project);
    }

    // Request upgrading existing building
    pub fn queue_upgrade(
        &mut self,
        project_id: u32,
        target_instance_id: u32,
        new_definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) {
        let project = Project::new(
            project_id,
            ProjectResult::UpgradeBuilding {
                target_instance_id,
                new_definition,
            },
            cost,
        );

        self.project_manager.add_project(project);
    }

    pub fn queue_claim_tile(
        &mut self,
        project_id: u32,
        target_position: Point,
        cost: ProjectCost,
    ) {
        let project = Project::new(
            project_id,
            ProjectResult::ClaimTile { target_position },
            cost,
        );

        self.project_manager.add_project(project);
    }
}


