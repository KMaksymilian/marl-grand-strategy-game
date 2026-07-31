use crate::domain::buildings::core::{Building, BuildingLocation};
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::economy::resource_allocation::AllocationEngine;
use crate::domain::settlement::building_manager::BuildingManager;
use crate::domain::settlement::demographics::{ConsumptionResult, NeedRegistry, PopulationManager};
use crate::domain::settlement::project_manager::ProjectManager;
use crate::domain::settlement::projects::{Project, ProjectCost, ProjectResult};
use crate::domain::settlement::settlement_inventory_manager::SettlementInventoryManager;
use std::sync::Arc;
use crate::domain::world_data::point::Point;


pub enum  SettlementCommand{
    ClaimTile { settlement_id: u32, target_position: Point },
}
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

    pub fn process_turn(&mut self) -> (ConsumptionResult, Vec<SettlementCommand>) {
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

        // 3. Consumption and production phase
        let consumption_result = self
            .population_manager
            .process_allocation(allocations.get(&0));
        self.project_manager.process_allocation(&allocations);
        let (produced, returned) = self.building_manager.process_allocations(&allocations);

        // 4. Collecting phase
        self.inventory_manager
            .process_production_results(produced, returned);
        self.inventory_manager.commit_pending_production();

        // 5. Project finalization phase
        let commands = self.resolve_completed_projects();

        (consumption_result, commands)
    }

    fn resolve_completed_projects(&mut self) -> Vec<SettlementCommand> {
        let completed = self.project_manager.extract_completed();
        let mut commands = Vec::new();

        for project in completed {
            match project.result {
                ProjectResult::ConstructBuilding {
                    location,
                    position,
                    definition,
                } => {
                    let new_building = Building::new(project.id, position, location, definition);
                    self.building_manager.construct_building(new_building);
                }
                ProjectResult::UpgradeBuilding {
                    target_instance_id,
                    new_definition,
                } => {
                    self.building_manager
                        .upgrade_building(target_instance_id, new_definition);
                }
                ProjectResult::RecruitUnit { .. } => {
                    // todo
                }
                ProjectResult::ClaimTile { target_position } => {
                    commands.push(SettlementCommand::ClaimTile {
                        settlement_id: self.id,
                        target_position,
                    });
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


