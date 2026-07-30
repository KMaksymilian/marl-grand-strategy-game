use crate::domain::buildings::core::{Building, BuildingLocation, Position};
use crate::domain::economy::resource_allocation::AllocationEngine;
use crate::domain::settlement::demographics::{ConsumptionResult, NeedRegistry, PopulationManager};
use crate::domain::settlement::projects::{Project, ProjectCost, ProjectResult};
use std::sync::Arc;
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::settlement::building_manager::BuildingManager;
use crate::domain::settlement::project_manager::ProjectManager;
use crate::domain::settlement::settlement_inventory_manager::SettlementInventoryManager;

#[derive(Debug)]
pub struct Settlement {
    pub id: u32,
    pub inventory_manager: SettlementInventoryManager,
    pub building_manager: BuildingManager,
    pub population_manager: PopulationManager,
    pub project_manager: ProjectManager,
}

impl Settlement {
    pub fn new(id: u32, initial_capacity: u32, initial_population: u32, need_registry: Arc<NeedRegistry>) -> Self {
        Self {
            id,
            inventory_manager: SettlementInventoryManager::new(initial_capacity),
            building_manager: BuildingManager::new(),
            population_manager: PopulationManager::new(initial_population, need_registry),
            project_manager: ProjectManager::new(),
        }
    }

    pub fn process_turn(&mut self) -> ConsumptionResult {
        // 0. Aktualizacja siły roboczej przed rozpoczęciem popytu
        self.inventory_manager.refresh_labor_capacity(self.population_manager.population);

        // 1. FAZA POPYTU
        let mut requests = Vec::new();
        requests.extend(self.population_manager.generate_requests());
        requests.extend(self.project_manager.generate_requests());
        requests.extend(self.building_manager.generate_requests());

        // 2. FAZA ALOKACJI (Zwróć uwagę na wypożyczenie main z inventory_manager)
        let allocations = AllocationEngine::execute(&mut self.inventory_manager.main, requests);

        // 3. FAZA KONSUMPCJI / PRODUKCJI
        let consumption_result = self.population_manager.process_allocation(allocations.get(&0));
        self.project_manager.process_allocation(&allocations);
        let (produced, returned) = self.building_manager.process_allocations(&allocations);

        // 4. FAZA MAGAZYNOWANIA
        self.inventory_manager.process_production_results(produced, returned);
        self.inventory_manager.commit_pending_production();

        // 5. FAZA FINALIZACJI
        self.resolve_completed_projects();

        consumption_result
    }

    fn resolve_completed_projects(&mut self) {
        let completed = self.project_manager.extract_completed();

        for project in completed {
            match project.result {
                ProjectResult::ConstructBuilding { location, position, definition } => {
                    // Projekt pamięta swoją docelową pozycję!
                    let new_building = Building::new(
                        project.id, // Warto później rozdzielić ID projektu od ID budynku
                        position,   // <- Podpinamy zapisaną pozycję
                        location,
                        definition
                    );
                    self.building_manager.construct_building(new_building);
                },
                ProjectResult::UpgradeBuilding { target_instance_id, new_definition } => {
                    self.building_manager.upgrade_building(target_instance_id, new_definition);
                },
                ProjectResult::RecruitUnit { .. } => {
                    // todo
                }
            }
        }
    }

    pub fn queue_construction(
        &mut self,
        project_id: u32,
        location: BuildingLocation,
        position: Position, // Przyjmujemy pozycję z zewnątrz
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) {
        let project = Project::new(
            project_id,
            ProjectResult::ConstructBuilding {
                location,
                position,
                definition
            },
            cost,
        );

        self.project_manager.add_project(project);
    }

    /// Zleca ulepszenie istniejącego budynku (np. z Tier 1 na Tier 2)
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
                new_definition
            },
            cost,
        );

        self.project_manager.add_project(project);
    }
}