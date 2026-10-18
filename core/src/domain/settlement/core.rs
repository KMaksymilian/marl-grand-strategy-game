use crate::domain::buildings::core::{Building, Position};
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::buildings::productivity::BuildingProductivityCalculator;
use crate::domain::economy::gold_inventory::GoldInventory;
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
use crate::domain::trade::trade_allocation_engine::TradeAllocationEngine;
use crate::domain::trade::trade_log::TradeLog;
use crate::domain::world_data::map::Map;
use crate::services::algorithms::path_finder::PathFinder;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingCommandError {
    MissingBuilding(BuildingId),
    InvalidLoad,
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
    gold_inventory: GoldInventory,
    inventory_manager: SettlementInventoryManager,
    trade_log: TradeLog,
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
        self.building_manager.buildings.iter()
    }
    pub fn active_projects(&self) -> &[Project] {
        &self.project_manager.active_projects
    }
    pub fn add_building(
        &mut self,
        building: Building,
        map: &Map,
        finder: &mut PathFinder,
    ) -> Result<(), BuildingId> {
        self.building_manager
            .construct_building(building, map, finder)
    }

    pub fn building_productivity_calculator(&self) -> Option<BuildingProductivityCalculator> {
        self.building_manager.productivity_calculator()
    }

    /// Recomputes cached multipliers immediately; `None` restores full productivity.
    pub fn set_building_productivity_calculator(
        &mut self,
        calculator: Option<BuildingProductivityCalculator>,
        map: &Map,
        finder: &mut PathFinder,
    ) {
        self.building_manager
            .set_productivity_calculator(calculator, map, finder);
    }
    pub fn set_building_load(
        &mut self,
        id: BuildingId,
        value: f32,
    ) -> Result<(), BuildingCommandError> {
        self.building_manager
            .buildings
            .iter_mut()
            .find(|building| building.instance_id() == id)
            .ok_or(BuildingCommandError::MissingBuilding(id))?
            .set_target_load(value)
            .map_err(|_| BuildingCommandError::InvalidLoad)
    }
    pub fn set_building_active(
        &mut self,
        id: BuildingId,
        active: bool,
    ) -> Result<(), BuildingCommandError> {
        self.building_manager
            .buildings
            .iter_mut()
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
            gold_inventory: GoldInventory::default(),
            inventory_manager: SettlementInventoryManager::new(initial_capacity),
            trade_log: TradeLog::default(),
            building_manager: BuildingManager::new(),
            population_manager: PopulationManager::new(initial_population, need_registry),
            project_manager: ProjectManager::new(),
        }
    }

    /// Refreshes route costs from the current map before any economic allocation.
    pub fn process_turn(&mut self, map: &Map, finder: &mut PathFinder) -> TurnReport {
        self.building_manager.refresh_productivity(map, finder);
        let opening_stock = InventoryTurnReport::snapshot(
            &self.inventory_manager.main,
            &self.inventory_manager.pending_production,
        );
        // Invalid projects must not participate in material allocation.
        let mut projects = self.reject_invalid_projects();
        // Labour update
        self.inventory_manager
            .refresh_labor_capacity(self.population_manager.population);

        // 0. TradeLog phase
        let demands: (HashMap<ResourceType, u32>, u32) = self.trade_log.calculate_demand();
        TradeAllocationEngine::execute(
            &mut self.trade_log,
            &mut self.gold_inventory,
            &mut self.inventory_manager.main,
            demands,
        );

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
        projects.extend(self.resolve_completed_projects(map, finder));

        // 6. TradeLog remaining resources phase
        let returned: (HashMap<ResourceType, u32>, u32) = self.trade_log.return_resources();
        TradeAllocationEngine::execute_remained_resources(
            &mut self.gold_inventory,
            &mut self.inventory_manager.main,
            returned,
        );

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

    fn resolve_completed_projects(
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

    fn finalize_project(
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

    // Request new building
    pub fn queue_construction(
        &mut self,
        project_id: ProjectId,
        position: Position,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    ) -> Result<(), ProjectError> {
        let project = Project::new(
            project_id,
            ProjectResult::ConstructBuilding {
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

#[cfg(test)]
mod tests {
    use crate::domain::geography::area as test_area;
    use crate::domain::world_data::map as core_map;
    use crate::services::algorithms::path_finder as test_path;

    fn test_map() -> core_map::Map {
        core_map::Map::new(
            64,
            8,
            vec![test_area::Area::new(0.5, 0.5); 64 * 8],
            vec![0; 64 * 8],
        )
    }

    fn test_finder() -> test_path::PathFinder {
        test_path::PathFinder::new(64, 8)
    }

    use super::*;
    use crate::domain::buildings::factory::BuildingBehavior;
    use crate::domain::economy::resource::ResourceType;
    use crate::domain::settlement::demographics::NeedTier;
    use crate::domain::settlement::turn_report::{InventoryLoss, InventoryLossStage};
    use std::collections::HashMap;

    fn definition(behaviors: Vec<BuildingBehavior>) -> Arc<BuildingDefinition> {
        Arc::new(BuildingDefinition {
            id: "test".into(),
            name: "Test".into(),
            tier: 1,
            max_workers: 1,
            behaviors,
            construction_cost: ProjectCost::default(),
        })
    }

    fn recipe(input: ResourceType, output: ResourceType) -> BuildingBehavior {
        BuildingBehavior::Production {
            inputs: vec![(input, 1)],
            outputs: vec![(output, 1)],
        }
    }

    fn add_building(settlement: &mut Settlement, id: u32, definition: Arc<BuildingDefinition>) {
        let mut building =
            Building::new(BuildingId(id), Position { x: 0, y: 0 }, definition).unwrap();
        building.set_target_load(1.0).unwrap();
        settlement
            .building_manager
            .construct_building(building, &test_map(), &mut test_finder())
            .unwrap();
    }

    fn stock(settlement: &Settlement, resource: ResourceType) -> u32 {
        settlement
            .inventory_manager
            .main
            .resources()
            .get(&resource)
            .copied()
            .unwrap_or(0)
    }

    #[test]
    fn storage_priorities_are_local_to_each_settlement() {
        let mut food_first = Settlement::new(1, 3, 1, Arc::default());
        let mut wood_first = Settlement::new(2, 3, 1, Arc::default());
        food_first
            .set_storage_order([ResourceType::Grain, ResourceType::Wood, ResourceType::Stone])
            .unwrap();
        wood_first
            .set_storage_order([ResourceType::Wood, ResourceType::Stone, ResourceType::Grain])
            .unwrap();
        let blueprint = definition(vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(ResourceType::Wood, 3), (ResourceType::Grain, 3)],
        }]);
        add_building(&mut food_first, 1, blueprint.clone());
        add_building(&mut wood_first, 1, blueprint);
        let food_report = food_first.process_turn(&test_map(), &mut test_finder());
        let wood_report = wood_first.process_turn(&test_map(), &mut test_finder());
        assert_eq!(food_first.storage_order()[0], ResourceType::Grain);
        assert_eq!(wood_first.storage_order()[0], ResourceType::Wood);
        assert_eq!(stock(&food_first, ResourceType::Grain), 3);
        assert_eq!(stock(&food_first, ResourceType::Wood), 0);
        assert_eq!(stock(&wood_first, ResourceType::Wood), 3);
        assert_eq!(stock(&wood_first, ResourceType::Grain), 0);
        assert!(food_report.inventory.is_balanced());
        assert!(wood_report.inventory.is_balanced());
    }

    #[test]
    fn changing_storage_priority_affects_future_admissions_without_eviction() {
        let mut settlement = Settlement::new(1, 3, 1, Arc::default());
        settlement
            .set_storage_order([ResourceType::Grain, ResourceType::Wood, ResourceType::Stone])
            .unwrap();
        add_building(
            &mut settlement,
            1,
            definition(vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(ResourceType::Wood, 3), (ResourceType::Grain, 3)],
            }]),
        );
        assert!(
            settlement
                .process_turn(&test_map(), &mut test_finder())
                .inventory
                .is_balanced()
        );
        assert_eq!(stock(&settlement, ResourceType::Grain), 3);

        settlement
            .set_storage_order([ResourceType::Wood, ResourceType::Stone, ResourceType::Grain])
            .unwrap();
        let report = settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(stock(&settlement, ResourceType::Grain), 3);
        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert!(report.inventory.is_balanced());

        assert!(
            settlement
                .inventory_manager
                .main
                .consume(ResourceType::Grain, 3)
        );
        assert!(
            settlement
                .process_turn(&test_map(), &mut test_finder())
                .inventory
                .is_balanced()
        );
        assert_eq!(stock(&settlement, ResourceType::Wood), 3);
        assert_eq!(stock(&settlement, ResourceType::Grain), 0);
    }

    #[test]
    fn buffer_and_warehouse_losses_are_reported_without_double_counting() {
        let mut settlement = Settlement::new(1, 5, 1, Arc::default());
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Stone, 3);
        add_building(
            &mut settlement,
            1,
            definition(vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(ResourceType::Wood, 4), (ResourceType::Grain, 4)],
            }]),
        );
        let report = settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(
            report.inventory.losses,
            vec![
                InventoryLoss {
                    resource: ResourceType::Grain,
                    amount: 3,
                    stage: InventoryLossStage::ProductionBuffer
                },
                InventoryLoss {
                    resource: ResourceType::Wood,
                    amount: 2,
                    stage: InventoryLossStage::Warehouse
                },
                InventoryLoss {
                    resource: ResourceType::Grain,
                    amount: 1,
                    stage: InventoryLossStage::Warehouse
                },
            ]
        );
        assert_eq!(stock(&settlement, ResourceType::Stone), 3);
        assert_eq!(stock(&settlement, ResourceType::Wood), 2);
        assert_eq!(report.inventory.produced[&ResourceType::Grain], 4);
        assert!(report.inventory.is_balanced());
        let mut corrupted = report.inventory;
        corrupted.closing_stock.insert(ResourceType::Wood, 3);
        assert!(!corrupted.is_balanced());
    }

    #[test]
    fn material_grants_returned_without_workers_are_not_consumption() {
        let mut settlement = Settlement::new(1, 2, 0, Arc::default());
        settlement.inventory_manager.main.add(ResourceType::Wood, 2);
        add_building(
            &mut settlement,
            1,
            definition(vec![BuildingBehavior::Production {
                inputs: vec![(ResourceType::Wood, 2)],
                outputs: vec![(ResourceType::Stone, 5)],
            }]),
        );
        let report = settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(report.inventory.returned[&ResourceType::Wood], 2);
        assert_eq!(report.inventory.production_consumed[&ResourceType::Wood], 0);
        assert!(report.inventory.produced.is_empty());
        assert!(report.inventory.losses.is_empty());
        assert_eq!(stock(&settlement, ResourceType::Wood), 2);
        assert!(report.inventory.is_balanced());
    }

    #[test]
    fn combined_building_production_can_exceed_u32_without_wrapping() {
        let mut settlement = Settlement::new(1, u32::MAX, 2, Arc::default());
        let blueprint = definition(vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(ResourceType::Wood, u32::MAX)],
        }]);
        add_building(&mut settlement, 1, blueprint.clone());
        add_building(&mut settlement, 2, blueprint);
        let report = settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(
            report.inventory.produced[&ResourceType::Wood],
            2 * u64::from(u32::MAX)
        );
        assert_eq!(
            report.inventory.losses,
            vec![InventoryLoss {
                resource: ResourceType::Wood,
                amount: u64::from(u32::MAX),
                stage: InventoryLossStage::ProductionBuffer,
            }]
        );
        assert_eq!(stock(&settlement, ResourceType::Wood), u32::MAX);
        assert!(report.inventory.is_balanced());
    }

    #[test]
    fn opening_balance_includes_preexisting_buffer_stock() {
        let mut settlement = Settlement::new(1, 3, 0, Arc::default());
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Grain, 1);
        settlement
            .inventory_manager
            .pending_production
            .add(ResourceType::Wood, 3);
        let report = settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(report.inventory.opening_stock[&ResourceType::Wood], 3);
        assert_eq!(report.inventory.closing_stock[&ResourceType::Wood], 2);
        assert_eq!(
            report.inventory.losses,
            vec![InventoryLoss {
                resource: ResourceType::Wood,
                amount: 1,
                stage: InventoryLossStage::Warehouse,
            }]
        );
        assert!(
            settlement
                .inventory_manager
                .pending_production
                .resources()
                .is_empty()
        );
        assert!(report.inventory.is_balanced());
    }

    #[test]
    fn complete_overflow_reports_are_independent_of_building_order() {
        let mut expected = None;
        for order in [[1, 2], [2, 1]] {
            let mut settlement = Settlement::new(1, 3, 2, Arc::default());
            for id in order {
                let output = if id == 1 {
                    ResourceType::Stone
                } else {
                    ResourceType::Wood
                };
                add_building(
                    &mut settlement,
                    id,
                    definition(vec![BuildingBehavior::Production {
                        inputs: vec![],
                        outputs: vec![(output, 3)],
                    }]),
                );
            }
            let report = settlement.process_turn(&test_map(), &mut test_finder());
            assert!(report.inventory.is_balanced());
            if let Some(expected) = &expected {
                assert_eq!(&report, expected);
            } else {
                expected = Some(report);
            }
            assert_eq!(stock(&settlement, ResourceType::Wood), 3);
            assert_eq!(stock(&settlement, ResourceType::Stone), 0);
        }
    }

    #[test]
    fn zero_cost_entries_do_not_block_construction() {
        let mut settlement = Settlement::new(1, 100, 0, Arc::default());
        settlement
            .queue_construction(
                ProjectId(10),
                Position { x: 0, y: 0 },
                definition(vec![]),
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 0)]),
                    turn_cost: 0,
                },
            )
            .unwrap();
        let report = settlement.process_turn(&test_map(), &mut test_finder());
        assert!(settlement.project_manager.active_projects.is_empty());
        assert_eq!(settlement.building_manager.buildings.len(), 1);
        assert_eq!(
            report.projects,
            vec![ProjectOutcome {
                project_id: ProjectId(10),
                result: Ok(ProjectCompletion::BuildingConstructed(BuildingId(0))),
            }]
        );
        assert!(
            settlement
                .process_turn(&test_map(), &mut test_finder())
                .projects
                .is_empty()
        );
    }

    #[test]
    fn missing_upgrade_target_is_rejected_without_spending_resources() {
        let mut settlement = Settlement::new(1, 100, 0, Arc::default());
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Wood, 10);
        let result = settlement.queue_upgrade(
            ProjectId(10),
            BuildingId(99),
            definition(vec![]),
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 5)]),
                turn_cost: 1,
            },
        );
        assert_eq!(result, Err(ProjectError::MissingBuilding(BuildingId(99))));
        assert!(settlement.project_manager.active_projects.is_empty());
        settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(stock(&settlement, ResourceType::Wood), 10);
    }

    #[test]
    fn upgrade_completion_reports_the_existing_building_id() {
        let mut settlement = Settlement::new(1, 100, 0, Arc::default());
        let old = definition(vec![]);
        let upgraded = definition(vec![]);
        settlement
            .building_manager
            .construct_building(
                Building::new(BuildingId(8), Position { x: 2, y: 3 }, old).unwrap(),
                &test_map(),
                &mut test_finder(),
            )
            .unwrap();
        settlement
            .queue_upgrade(
                ProjectId(4),
                BuildingId(8),
                upgraded.clone(),
                ProjectCost {
                    needed_resources: HashMap::from([
                        (ResourceType::Wood, 2),
                        (ResourceType::Stone, 0),
                    ]),
                    turn_cost: 1,
                },
            )
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 2);

        let report = settlement.process_turn(&test_map(), &mut test_finder());

        assert_eq!(
            report.projects,
            vec![ProjectOutcome {
                project_id: ProjectId(4),
                result: Ok(ProjectCompletion::BuildingUpgraded(BuildingId(8))),
            }]
        );
        let building = &settlement.building_manager.buildings[0];
        assert_eq!(building.instance_id(), BuildingId(8));
        assert_eq!(building.position(), Position { x: 2, y: 3 });
        assert!(std::ptr::eq(building.definition(), upgraded.as_ref()));
        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert!(settlement.project_manager.active_projects.is_empty());
        assert!(
            settlement
                .process_turn(&test_map(), &mut test_finder())
                .projects
                .is_empty()
        );
    }

    #[test]
    fn removed_upgrade_target_fails_before_further_material_allocation() {
        let mut settlement = Settlement::new(1, 100, 0, Arc::default());
        add_building(&mut settlement, 1, definition(vec![]));
        settlement
            .queue_upgrade(
                ProjectId(5),
                BuildingId(1),
                definition(vec![]),
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 3)]),
                    turn_cost: 2,
                },
            )
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);
        assert!(
            settlement
                .process_turn(&test_map(), &mut test_finder())
                .projects
                .is_empty()
        );
        assert_eq!(
            settlement.project_manager.active_projects[0]
                .remaining_cost
                .needed_resources[&ResourceType::Wood],
            2
        );

        settlement.building_manager.buildings.clear();
        settlement.inventory_manager.main.add(ResourceType::Wood, 2);
        let report = settlement.process_turn(&test_map(), &mut test_finder());

        assert_eq!(
            report.projects,
            vec![ProjectOutcome {
                project_id: ProjectId(5),
                result: Err(ProjectError::MissingBuilding(BuildingId(1))),
            }]
        );
        assert_eq!(stock(&settlement, ResourceType::Wood), 2);
        assert!(settlement.project_manager.active_projects.is_empty());
        assert!(
            settlement
                .process_turn(&test_map(), &mut test_finder())
                .projects
                .is_empty()
        );
    }

    #[test]
    fn unsupported_recruitment_reports_failure_without_spending() {
        let mut settlement = Settlement::new(1, 100, 0, Arc::default());
        settlement
            .project_manager
            .add_project(Project::new(
                ProjectId(3),
                ProjectResult::RecruitUnit {},
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 2)]),
                    turn_cost: 1,
                },
            ))
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 2);
        let report = settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(
            report.projects,
            vec![ProjectOutcome {
                project_id: ProjectId(3),
                result: Err(ProjectError::UnsupportedRecruitment),
            }]
        );
        assert_eq!(stock(&settlement, ResourceType::Wood), 2);
        assert!(settlement.project_manager.active_projects.is_empty());
    }

    #[test]
    fn pending_construction_reports_completion_only_after_cost_and_time_are_paid() {
        let mut settlement = Settlement::new(1, 100, 0, Arc::default());
        settlement
            .queue_construction(
                ProjectId(2),
                Position { x: 0, y: 0 },
                definition(vec![]),
                ProjectCost {
                    needed_resources: HashMap::from([
                        (ResourceType::Wood, 2),
                        (ResourceType::Stone, 0),
                    ]),
                    turn_cost: 2,
                },
            )
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);
        assert!(
            settlement
                .process_turn(&test_map(), &mut test_finder())
                .projects
                .is_empty()
        );
        assert_eq!(
            settlement.project_manager.active_projects[0]
                .remaining_cost
                .turn_cost,
            1
        );
        assert!(
            settlement
                .process_turn(&test_map(), &mut test_finder())
                .projects
                .is_empty()
        );
        assert_eq!(
            settlement.project_manager.active_projects[0]
                .remaining_cost
                .turn_cost,
            0
        );
        assert!(settlement.building_manager.buildings.is_empty());

        settlement.inventory_manager.main.add(ResourceType::Wood, 1);
        let report = settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(
            report.projects,
            vec![ProjectOutcome {
                project_id: ProjectId(2),
                result: Ok(ProjectCompletion::BuildingConstructed(BuildingId(0))),
            }]
        );
        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
    }

    #[test]
    fn duplicate_queue_requests_return_an_explicit_error() {
        let mut settlement = Settlement::new(1, 100, 0, Arc::default());
        add_building(&mut settlement, 1, definition(vec![]));
        settlement
            .queue_construction(
                ProjectId(2),
                Position { x: 0, y: 0 },
                definition(vec![]),
                ProjectCost::default(),
            )
            .unwrap();
        assert_eq!(
            settlement.queue_upgrade(
                ProjectId(2),
                BuildingId(1),
                definition(vec![]),
                ProjectCost::default()
            ),
            Err(ProjectError::DuplicateProject(ProjectId(2)))
        );
        assert_eq!(settlement.project_manager.active_projects.len(), 1);
    }

    #[test]
    fn finalization_checks_upgrade_targets_again() {
        let mut settlement = Settlement::new(1, 100, 0, Arc::default());
        assert_eq!(
            settlement.finalize_project(
                ProjectResult::UpgradeBuilding {
                    target_instance_id: BuildingId(42),
                    new_definition: definition(vec![]),
                },
                &test_map(),
                &mut test_finder()
            ),
            Err(ProjectError::MissingBuilding(BuildingId(42)))
        );
    }

    #[test]
    fn invalid_projects_do_not_take_materials_from_valid_construction() {
        let mut settlement = Settlement::new(1, 100, 0, Arc::default());
        settlement
            .project_manager
            .add_project(Project::new(
                ProjectId(20),
                ProjectResult::UpgradeBuilding {
                    target_instance_id: BuildingId(99),
                    new_definition: definition(vec![]),
                },
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                    turn_cost: 1,
                },
            ))
            .unwrap();
        settlement
            .queue_construction(
                ProjectId(21),
                Position { x: 0, y: 0 },
                definition(vec![]),
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                    turn_cost: 1,
                },
            )
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);

        let report = settlement.process_turn(&test_map(), &mut test_finder());

        assert_eq!(
            report.projects,
            vec![
                ProjectOutcome {
                    project_id: ProjectId(20),
                    result: Err(ProjectError::MissingBuilding(BuildingId(99)))
                },
                ProjectOutcome {
                    project_id: ProjectId(21),
                    result: Ok(ProjectCompletion::BuildingConstructed(BuildingId(0)))
                },
            ]
        );
        assert!(settlement.project_manager.active_projects.is_empty());
        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert_eq!(settlement.building_manager.buildings.len(), 1);
    }

    #[test]
    fn population_building_and_project_share_whole_resource_units() {
        let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(ResourceType::Wood, 1)]),
        }]));
        let mut settlement = Settlement::new(1, 100, 2, registry);
        let blueprint = definition(vec![recipe(ResourceType::Wood, ResourceType::Stone)]);
        add_building(&mut settlement, 1, blueprint.clone());
        settlement
            .queue_construction(
                ProjectId(1),
                Position { x: 1, y: 0 },
                blueprint,
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                    turn_cost: 1,
                },
            )
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 4);

        let result = settlement.process_turn(&test_map(), &mut test_finder());

        // Two residents consume 2, the recipe consumes 1, construction costs 1.
        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert_eq!(stock(&settlement, ResourceType::Stone), 1);
        assert_eq!(result.consumption.satisfaction_percent, 1.0);
        assert_eq!(result.inventory.population_consumed[&ResourceType::Wood], 2);
        assert_eq!(result.inventory.project_consumed[&ResourceType::Wood], 1);
        assert_eq!(result.inventory.production_consumed[&ResourceType::Wood], 1);
        assert!(result.inventory.losses.is_empty());
        assert!(result.inventory.is_balanced());
        assert!(result.consumption.missing_resources.is_empty());
        assert!(settlement.project_manager.active_projects.is_empty());
        assert_eq!(settlement.building_manager.buildings.len(), 2);
    }

    #[test]
    fn population_needs_use_the_same_units_as_inventory_in_a_full_turn() {
        let registry = Arc::new(NeedRegistry::new(vec![
            NeedTier {
                min_population: 1,
                needs_per_capita: HashMap::from([(ResourceType::Grain, 1)]),
            },
            NeedTier {
                min_population: 1,
                needs_per_capita: HashMap::from([(ResourceType::Grain, 1)]),
            },
        ]));
        let mut settlement = Settlement::new(1, 100, 1, registry);
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Grain, 3);

        let result = settlement.process_turn(&test_map(), &mut test_finder());

        assert_eq!(result.consumption.satisfaction_percent, 1.0);
        assert!(result.consumption.missing_resources.is_empty());
        assert_eq!(stock(&settlement, ResourceType::Grain), 1);
    }

    #[test]
    fn population_shortage_is_reported_across_turns() {
        let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(ResourceType::Grain, 1), (ResourceType::Wood, 1)]),
        }]));
        let mut settlement = Settlement::new(1, 100, 1, registry);
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Grain, 3);

        for remaining in [2, 1] {
            let result = settlement.process_turn(&test_map(), &mut test_finder());
            assert_eq!(result.consumption.satisfaction_percent, 0.5);
            assert_eq!(
                result.consumption.missing_resources,
                vec![ResourceType::Wood]
            );
            assert_eq!(stock(&settlement, ResourceType::Grain), remaining);
        }
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);
        let result = settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(result.consumption.satisfaction_percent, 1.0);
        assert!(result.consumption.missing_resources.is_empty());
        assert_eq!(stock(&settlement, ResourceType::Grain), 0);
        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
    }

    #[test]
    fn building_and_project_with_same_number_have_separate_allocations() {
        let mut settlement = Settlement::new(1, 100, 1, Arc::default());
        let blueprint = definition(vec![recipe(ResourceType::Wood, ResourceType::Stone)]);
        add_building(&mut settlement, 7, blueprint.clone());
        settlement
            .queue_construction(
                ProjectId(7),
                Position { x: 1, y: 0 },
                blueprint,
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                    turn_cost: 1,
                },
            )
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 2);

        settlement.process_turn(&test_map(), &mut test_finder());

        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert_eq!(stock(&settlement, ResourceType::Stone), 1);
        assert!(settlement.project_manager.active_projects.is_empty());
        let buildings = &settlement.building_manager.buildings;
        assert_eq!(buildings.len(), 2);
        assert_ne!(buildings[0].instance_id(), buildings[1].instance_id());
    }

    #[test]
    fn population_and_building_zero_have_separate_allocations() {
        let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(ResourceType::Grain, 1)]),
        }]));
        let mut settlement = Settlement::new(1, 100, 1, registry);
        add_building(
            &mut settlement,
            0,
            definition(vec![recipe(ResourceType::Grain, ResourceType::Stone)]),
        );
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Grain, 2);

        let report = settlement.process_turn(&test_map(), &mut test_finder());

        assert_eq!(report.consumption.satisfaction_percent, 1.0);
        assert_eq!(
            report.inventory.population_consumed[&ResourceType::Grain],
            1
        );
        assert!(report.inventory.project_consumed.is_empty());
        assert_eq!(
            report.inventory.production_consumed[&ResourceType::Grain],
            1
        );
        assert!(report.inventory.losses.is_empty());
        assert!(report.inventory.is_balanced());
        assert_eq!(stock(&settlement, ResourceType::Grain), 0);
        assert_eq!(stock(&settlement, ResourceType::Stone), 1);
    }

    #[test]
    fn shared_recipe_inputs_are_consumed_only_once() {
        let mut settlement = Settlement::new(1, 100, 1, Arc::default());
        add_building(
            &mut settlement,
            1,
            definition(vec![
                recipe(ResourceType::Wood, ResourceType::Stone),
                recipe(ResourceType::Wood, ResourceType::Grain),
            ]),
        );
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);

        settlement.process_turn(&test_map(), &mut test_finder());

        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert_eq!(stock(&settlement, ResourceType::Stone), 1);
        assert_eq!(stock(&settlement, ResourceType::Grain), 0);
    }

    #[test]
    fn remaining_single_material_advances_one_of_two_projects() {
        let mut settlement = Settlement::new(1, 100, 0, Arc::default());
        for id in [1, 2] {
            settlement
                .queue_construction(
                    ProjectId(id),
                    Position { x: 0, y: 0 },
                    definition(vec![]),
                    ProjectCost {
                        needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                        turn_cost: 1,
                    },
                )
                .unwrap();
        }
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);

        settlement.process_turn(&test_map(), &mut test_finder());

        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert_eq!(settlement.building_manager.buildings.len(), 1);
        assert_eq!(settlement.project_manager.active_projects.len(), 1);
        assert_eq!(
            settlement.project_manager.active_projects[0].id,
            ProjectId(2)
        );
    }

    #[test]
    fn large_project_cost_is_deducted_exactly() {
        let amount = 16_777_219;
        let mut settlement = Settlement::new(1, u32::MAX, 0, Arc::default());
        settlement
            .queue_construction(
                ProjectId(1),
                Position { x: 0, y: 0 },
                definition(vec![]),
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, amount)]),
                    turn_cost: 1,
                },
            )
            .unwrap();
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Wood, amount);

        settlement.process_turn(&test_map(), &mut test_finder());

        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert_eq!(settlement.building_manager.buildings.len(), 1);
    }

    #[test]
    fn production_is_available_for_consumption_in_the_next_turn() {
        let mut settlement = Settlement::new(1, 100, 2, Arc::default());
        add_building(
            &mut settlement,
            1,
            definition(vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(ResourceType::Grain, 1)],
            }]),
        );
        add_building(
            &mut settlement,
            2,
            definition(vec![recipe(ResourceType::Grain, ResourceType::Stone)]),
        );

        settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(stock(&settlement, ResourceType::Grain), 1);
        assert_eq!(stock(&settlement, ResourceType::Stone), 0);

        settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(stock(&settlement, ResourceType::Grain), 1);
        assert_eq!(stock(&settlement, ResourceType::Stone), 1);
        assert!(
            settlement
                .inventory_manager
                .pending_production
                .resources()
                .is_empty()
        );
    }
}
