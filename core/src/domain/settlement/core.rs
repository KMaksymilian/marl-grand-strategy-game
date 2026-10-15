use crate::domain::buildings::core::{Building, BuildingLocation, Position};
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::economy::gold::GoldInventoryManager;
use crate::domain::economy::resource::ResourceType;
use crate::domain::economy::resource_allocation::{AllocationEngine, AllocationRecipient};
use crate::domain::economy::storage_order::StorageOrderError;
use crate::domain::ids::{BuildingId, ProjectId};
use crate::domain::settlement::building_manager::BuildingManager;
use crate::domain::settlement::demographics::{NeedRegistry, PopulationManager};
use crate::domain::settlement::project_manager::ProjectManager;
use crate::domain::settlement::projects::{
    Project, ProjectCompletion, ProjectCost, ProjectError, ProjectOutcome, ProjectResult,
};
use crate::domain::settlement::settlement_inventory_manager::SettlementInventoryManager;
use crate::domain::settlement::turn_report::{InventoryTurnReport, TurnReport};
use crate::domain::trading::trade_log_demand::TradeLogDemand;
use crate::domain::trading::trade_log_manager::TradeLogManager;
use std::sync::Arc;

#[cfg(test)]
#[path = "turn_tests.rs"]
mod turn_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingCommandError {
    MissingBuilding(BuildingId),
    InvalidEfficiency,
}

/// Managers are implementation details; public views cannot mutate their collections.
/// ```compile_fail
/// use core::domain::settlement::core::Settlement;
/// let mut settlement = Settlement::new(1, 10, 1, std::sync::Arc::default());
/// settlement.project_manager.active_projects.clear();
/// ```
#[derive(Debug)]
pub struct Settlement {
    id: u32,
    gold_inventory: GoldInventoryManager,
    inventory_manager: SettlementInventoryManager,
    trade_log_manager: TradeLogManager,
    building_manager: BuildingManager,
    population_manager: PopulationManager,
    project_manager: ProjectManager,
}

impl Settlement {
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn population(&self) -> u32 {
        self.population_manager.population
    }
    pub fn gold(&self) -> u32 {
        self.gold_inventory.gold_amount
    }
    pub fn inventory(&self) -> &crate::domain::economy::resource::ResourceInventory {
        &self.inventory_manager.main
    }
    pub fn pending_inventory(&self) -> &crate::domain::economy::resource::ResourceInventory {
        &self.inventory_manager.pending_production
    }
    pub fn deposit_gold(&mut self, amount: u32) {
        self.gold_inventory.add(amount);
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
    pub fn withdraw_gold(&mut self, amount: u32) -> bool {
        self.gold_inventory.consume(amount)
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

    pub fn new(
        id: u32,
        initial_capacity: u32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
    ) -> Self {
        Self {
            id,
            gold_inventory: GoldInventoryManager::new(),
            inventory_manager: SettlementInventoryManager::new(initial_capacity),
            trade_log_manager: TradeLogManager::new(),
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

        // 1. TradeLog Update
        let trade_log_demands: Vec<TradeLogDemand> = self.trade_log_manager.generate_trade_log_demand();

        // 2. TradeLog Allocation
        self.trade_log_manager.trade_log = AllocationEngine::execute_trading_log(&mut self.inventory_manager.main, trade_log_demands);

        // 3. Caravan setup (if possible)

        // 4. Caravan Allocation (if possible)

        // 4. Demand phase
        let mut requests = Vec::new();
        requests.extend(self.population_manager.generate_requests());
        requests.extend(self.project_manager.generate_requests());
        requests.extend(self.building_manager.generate_requests());

        // 5. Allocation phase
        let allocations = AllocationEngine::execute(&mut self.inventory_manager.main, requests);
        let mut inventory = InventoryTurnReport::from_allocations(opening_stock, &allocations);

        // 6. Consumption and production phase
        let consumption_result = self
            .population_manager
            .process_allocation(allocations.get(&AllocationRecipient::Population));
        self.project_manager.process_allocation(&allocations);
        let (produced, returned) = self.building_manager.process_allocations(&allocations);
        inventory.record_production(&produced, &returned);

        // 7. Collecting phase
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

        // 8. Project finalization phase
        projects.extend(self.resolve_completed_projects());

        TurnReport {
            consumption: consumption_result,
            projects,
            inventory,
        }
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

    fn validate_project_result(
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
                let new_building = Building::new(id, position, location, definition)
                    .map_err(ProjectError::InvalidDefinition)?;
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
