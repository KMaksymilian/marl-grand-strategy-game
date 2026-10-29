//! Settlement phases shared by local turns and the global coordinator.
use super::Settlement;
use super::projects::ProjectOutcome;
use super::turn_report::{ConsumptionResult, InventoryTurnReport, MaterialTotals, TurnReport};
use crate::economy::ResourceType;
use crate::economy::allocation::{AllocationEngine, AllocationRecipient, AllocationResult};
use crate::trade::allocation::TradeAllocationEngine;
use crate::world::map::Map;
use crate::world::pathfinding::path_finder::PathFinder;
use crate::world::territory::TerritoryManager;
use std::collections::HashMap;

pub(crate) struct PreparedTurn {
    projects: Vec<ProjectOutcome>,
    opening_stock: MaterialTotals,
}

pub(crate) struct AllocatedTurn {
    projects: Vec<ProjectOutcome>,
    allocations: AllocationResult,
    inventory: InventoryTurnReport,
}

pub(crate) struct ConsumedTurn {
    allocated: AllocatedTurn,
    consumption: ConsumptionResult,
}

pub(crate) struct ProducedTurn {
    consumed: ConsumedTurn,
    produced: HashMap<ResourceType, u64>,
    returned: HashMap<ResourceType, u64>,
}

impl Settlement {
    /// Local convenience entry point using the same phases as Simulation.
    /// Reuse the same spatial registry supplied to construction commands.
    pub fn process_turn(
        &mut self,
        map: &Map,
        finder: &mut PathFinder,
        territory: &mut TerritoryManager,
    ) -> TurnReport {
        let prepared = self.prepare_turn(map, finder, territory);
        let allocated = self.allocate_turn(prepared);
        let consumed = self.consume_turn(allocated);
        self.progress_construction(&consumed);
        let produced = self.produce_turn(consumed);
        self.finish_turn(produced, map, finder, territory)
    }

    pub(crate) fn prepare_turn(
        &mut self,
        map: &Map,
        finder: &mut PathFinder,
        territory: &mut TerritoryManager,
    ) -> PreparedTurn {
        self.building_manager.refresh_productivity(map, finder);
        let opening_stock = InventoryTurnReport::snapshot(
            &self.inventory_manager.main,
            &self.inventory_manager.pending_production,
        );
        let projects = self.reject_invalid_projects(map, territory);
        self.inventory_manager
            .refresh_labor_capacity(self.population_manager.population);
        let demands = self.book.calculate_demand();
        TradeAllocationEngine::execute(
            &mut self.book,
            &mut self.gold_inventory,
            &mut self.inventory_manager.main,
            demands,
        );
        PreparedTurn {
            projects,
            opening_stock,
        }
    }

    pub(crate) fn allocate_turn(&mut self, prepared: PreparedTurn) -> AllocatedTurn {
        let mut requests = Vec::new();
        requests.extend(self.population_manager.generate_requests());
        requests.extend(self.project_manager.generate_requests());
        requests.extend(self.building_manager.generate_requests());
        let allocations = AllocationEngine::execute(&mut self.inventory_manager.main, requests);
        let inventory = InventoryTurnReport::from_allocations(prepared.opening_stock, &allocations);
        AllocatedTurn {
            projects: prepared.projects,
            allocations,
            inventory,
        }
    }

    pub(crate) fn consume_turn(&mut self, allocated: AllocatedTurn) -> ConsumedTurn {
        let consumption = self
            .population_manager
            .process_allocation(allocated.allocations.get(&AllocationRecipient::Population));
        ConsumedTurn {
            allocated,
            consumption,
        }
    }

    pub(crate) fn progress_construction(&mut self, consumed: &ConsumedTurn) {
        self.project_manager
            .process_allocation(&consumed.allocated.allocations);
    }

    pub(crate) fn produce_turn(&mut self, mut consumed: ConsumedTurn) -> ProducedTurn {
        let (produced, returned) = self
            .building_manager
            .process_allocations(&consumed.allocated.allocations);
        consumed
            .allocated
            .inventory
            .record_production(&produced, &returned);
        ProducedTurn {
            consumed,
            produced,
            returned,
        }
    }

    pub(crate) fn finish_turn(
        &mut self,
        produced: ProducedTurn,
        map: &Map,
        finder: &mut PathFinder,
        territory: &mut TerritoryManager,
    ) -> TurnReport {
        let ProducedTurn {
            consumed,
            produced,
            returned,
        } = produced;
        let ConsumedTurn {
            allocated,
            consumption,
        } = consumed;
        let AllocatedTurn {
            mut projects,
            mut inventory,
            ..
        } = allocated;
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
        projects.extend(self.resolve_completed_projects(map, finder, territory));
        // Preserve the existing local accounting boundary: trade returns occur
        // after the closing stock snapshot and are not covered by is_balanced().
        let returned = self.book.return_resources();
        TradeAllocationEngine::execute_remained_resources(
            &mut self.gold_inventory,
            &mut self.inventory_manager.main,
            returned,
        );
        TurnReport {
            consumption,
            projects,
            inventory,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ids::SettlementId;
    use crate::world::map as core_map;
    use crate::world::pathfinding::path_finder as test_path;
    use crate::world::terrain::area as test_area;

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
    use crate::buildings::Building;
    use crate::buildings::definition::BuildingBehavior;
    use crate::buildings::definition::BuildingDefinition;
    use crate::economy::ResourceType;
    use crate::ids::{BuildingId, ProjectId};
    use crate::settlement::NeedRegistry;
    use crate::settlement::population::NeedTier;
    use crate::settlement::projects::{Project, ProjectCost};
    use crate::settlement::projects::{
        ProjectCompletion, ProjectError, ProjectOutcome, ProjectResult,
    };
    use crate::settlement::turn_report::{InventoryLoss, InventoryLossStage};
    use crate::world::coordinates::Position;
    use std::collections::HashMap;
    use std::sync::Arc;

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

    fn add_building(
        settlement: &mut Settlement,
        id: u32,
        definition: Arc<BuildingDefinition>,
        territory: &mut TerritoryManager,
    ) {
        let mut building = Building::new(
            BuildingId(id),
            Position {
                x: id as i32,
                y: settlement.id().0 as i32,
            },
            definition,
        )
        .unwrap();
        building.set_target_load(1.0).unwrap();
        settlement
            .add_building(building, &test_map(), &mut test_finder(), territory)
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
        let mut territory = TerritoryManager::default();
        let mut food_first = Settlement::new(SettlementId(1), 3, 1, Arc::default());
        let mut wood_first = Settlement::new(SettlementId(2), 3, 1, Arc::default());
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
        add_building(&mut food_first, 1, blueprint.clone(), &mut territory);
        add_building(&mut wood_first, 1, blueprint, &mut territory);
        let food_report = food_first.process_turn(&test_map(), &mut test_finder(), &mut territory);
        let wood_report = wood_first.process_turn(&test_map(), &mut test_finder(), &mut territory);
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
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 3, 1, Arc::default());
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
            &mut territory,
        );
        assert!(
            settlement
                .process_turn(&test_map(), &mut test_finder(), &mut territory)
                .inventory
                .is_balanced()
        );
        assert_eq!(stock(&settlement, ResourceType::Grain), 3);

        settlement
            .set_storage_order([ResourceType::Wood, ResourceType::Stone, ResourceType::Grain])
            .unwrap();
        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
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
                .process_turn(&test_map(), &mut test_finder(), &mut territory)
                .inventory
                .is_balanced()
        );
        assert_eq!(stock(&settlement, ResourceType::Wood), 3);
        assert_eq!(stock(&settlement, ResourceType::Grain), 0);
    }

    #[test]
    fn buffer_and_warehouse_losses_are_reported_without_double_counting() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 5, 1, Arc::default());
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
            &mut territory,
        );
        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
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
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 2, 0, Arc::default());
        settlement.inventory_manager.main.add(ResourceType::Wood, 2);
        add_building(
            &mut settlement,
            1,
            definition(vec![BuildingBehavior::Production {
                inputs: vec![(ResourceType::Wood, 2)],
                outputs: vec![(ResourceType::Stone, 5)],
            }]),
            &mut territory,
        );
        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
        assert_eq!(report.inventory.returned[&ResourceType::Wood], 2);
        assert_eq!(report.inventory.production_consumed[&ResourceType::Wood], 0);
        assert!(report.inventory.produced.is_empty());
        assert!(report.inventory.losses.is_empty());
        assert_eq!(stock(&settlement, ResourceType::Wood), 2);
        assert!(report.inventory.is_balanced());
    }

    #[test]
    fn combined_building_production_can_exceed_u32_without_wrapping() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), u32::MAX, 2, Arc::default());
        let blueprint = definition(vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(ResourceType::Wood, u32::MAX)],
        }]);
        add_building(&mut settlement, 1, blueprint.clone(), &mut territory);
        add_building(&mut settlement, 2, blueprint, &mut territory);
        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
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
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 3, 0, Arc::default());
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Grain, 1);
        settlement
            .inventory_manager
            .pending_production
            .add(ResourceType::Wood, 3);
        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
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
            let mut territory = TerritoryManager::default();
            let mut settlement = Settlement::new(SettlementId(1), 3, 2, Arc::default());
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
                    &mut territory,
                );
            }
            let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
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
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
        settlement
            .queue_construction(
                ProjectId(10),
                Position { x: 0, y: 0 },
                definition(vec![]),
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 0)]),
                    turn_cost: 0,
                },
                &test_map(),
                &mut territory,
            )
            .unwrap();
        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
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
                .process_turn(&test_map(), &mut test_finder(), &mut territory)
                .projects
                .is_empty()
        );
    }

    #[test]
    fn missing_upgrade_target_is_rejected_without_spending_resources() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
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
        settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
        assert_eq!(stock(&settlement, ResourceType::Wood), 10);
    }

    #[test]
    fn upgrade_completion_reports_the_existing_building_id() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
        let old = definition(vec![]);
        let upgraded = definition(vec![]);
        settlement
            .add_building(
                Building::new(BuildingId(8), Position { x: 2, y: 3 }, old).unwrap(),
                &test_map(),
                &mut test_finder(),
                &mut territory,
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

        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);

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
                .process_turn(&test_map(), &mut test_finder(), &mut territory)
                .projects
                .is_empty()
        );
    }

    #[test]
    fn removed_upgrade_target_fails_before_further_material_allocation() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
        add_building(&mut settlement, 1, definition(vec![]), &mut territory);
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
                .process_turn(&test_map(), &mut test_finder(), &mut territory)
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
        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);

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
                .process_turn(&test_map(), &mut test_finder(), &mut territory)
                .projects
                .is_empty()
        );
    }

    #[test]
    fn unsupported_recruitment_reports_failure_without_spending() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
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
        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
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
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
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
                &test_map(),
                &mut territory,
            )
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);
        assert!(
            settlement
                .process_turn(&test_map(), &mut test_finder(), &mut territory)
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
                .process_turn(&test_map(), &mut test_finder(), &mut territory)
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
        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
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
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
        add_building(&mut settlement, 1, definition(vec![]), &mut territory);
        settlement
            .queue_construction(
                ProjectId(2),
                Position { x: 0, y: 0 },
                definition(vec![]),
                ProjectCost::default(),
                &test_map(),
                &mut territory,
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
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
        assert_eq!(
            settlement.finalize_project(
                ProjectId(0),
                ProjectResult::UpgradeBuilding {
                    target_instance_id: BuildingId(42),
                    new_definition: definition(vec![]),
                },
                &test_map(),
                &mut test_finder(),
                &mut territory
            ),
            Err(ProjectError::MissingBuilding(BuildingId(42)))
        );
    }

    #[test]
    fn invalid_projects_do_not_take_materials_from_valid_construction() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
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
                &test_map(),
                &mut territory,
            )
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);

        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);

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
        let mut territory = TerritoryManager::default();
        let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(ResourceType::Wood, 1)]),
        }]));
        let mut settlement = Settlement::new(SettlementId(1), 100, 2, registry);
        let blueprint = definition(vec![recipe(ResourceType::Wood, ResourceType::Stone)]);
        add_building(&mut settlement, 1, blueprint.clone(), &mut territory);
        settlement
            .queue_construction(
                ProjectId(1),
                Position { x: 1, y: 0 },
                blueprint,
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                    turn_cost: 1,
                },
                &test_map(),
                &mut territory,
            )
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 4);

        let result = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);

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
        let mut territory = TerritoryManager::default();
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
        let mut settlement = Settlement::new(SettlementId(1), 100, 1, registry);
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Grain, 3);

        let result = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);

        assert_eq!(result.consumption.satisfaction_percent, 1.0);
        assert!(result.consumption.missing_resources.is_empty());
        assert_eq!(stock(&settlement, ResourceType::Grain), 1);
    }

    #[test]
    fn population_shortage_is_reported_across_turns() {
        let mut territory = TerritoryManager::default();
        let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(ResourceType::Grain, 1), (ResourceType::Wood, 1)]),
        }]));
        let mut settlement = Settlement::new(SettlementId(1), 100, 1, registry);
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Grain, 3);

        for remaining in [2, 1] {
            let result = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
            assert_eq!(result.consumption.satisfaction_percent, 0.5);
            assert_eq!(
                result.consumption.missing_resources,
                vec![ResourceType::Wood]
            );
            assert_eq!(stock(&settlement, ResourceType::Grain), remaining);
        }
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);
        let result = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
        assert_eq!(result.consumption.satisfaction_percent, 1.0);
        assert!(result.consumption.missing_resources.is_empty());
        assert_eq!(stock(&settlement, ResourceType::Grain), 0);
        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
    }

    #[test]
    fn building_and_project_with_same_number_have_separate_allocations() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 1, Arc::default());
        let blueprint = definition(vec![recipe(ResourceType::Wood, ResourceType::Stone)]);
        add_building(&mut settlement, 7, blueprint.clone(), &mut territory);
        settlement
            .queue_construction(
                ProjectId(7),
                Position { x: 1, y: 0 },
                blueprint,
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                    turn_cost: 1,
                },
                &test_map(),
                &mut territory,
            )
            .unwrap();
        settlement.inventory_manager.main.add(ResourceType::Wood, 2);

        settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);

        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert_eq!(stock(&settlement, ResourceType::Stone), 1);
        assert!(settlement.project_manager.active_projects.is_empty());
        let buildings = &settlement.building_manager.buildings;
        assert_eq!(buildings.len(), 2);
        assert_ne!(buildings[0].instance_id(), buildings[1].instance_id());
    }

    #[test]
    fn population_and_building_zero_have_separate_allocations() {
        let mut territory = TerritoryManager::default();
        let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(ResourceType::Grain, 1)]),
        }]));
        let mut settlement = Settlement::new(SettlementId(1), 100, 1, registry);
        add_building(
            &mut settlement,
            0,
            definition(vec![recipe(ResourceType::Grain, ResourceType::Stone)]),
            &mut territory,
        );
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Grain, 2);

        let report = settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);

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
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 1, Arc::default());
        add_building(
            &mut settlement,
            1,
            definition(vec![
                recipe(ResourceType::Wood, ResourceType::Stone),
                recipe(ResourceType::Wood, ResourceType::Grain),
            ]),
            &mut territory,
        );
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);

        settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);

        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert_eq!(stock(&settlement, ResourceType::Stone), 1);
        assert_eq!(stock(&settlement, ResourceType::Grain), 0);
    }

    #[test]
    fn remaining_single_material_advances_one_of_two_projects() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
        for id in [1, 2] {
            settlement
                .queue_construction(
                    ProjectId(id),
                    Position { x: id as i32, y: 0 },
                    definition(vec![]),
                    ProjectCost {
                        needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                        turn_cost: 1,
                    },
                    &test_map(),
                    &mut territory,
                )
                .unwrap();
        }
        settlement.inventory_manager.main.add(ResourceType::Wood, 1);

        settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);

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
        let mut territory = TerritoryManager::default();
        let amount = 16_777_219;
        let mut settlement = Settlement::new(SettlementId(1), u32::MAX, 0, Arc::default());
        settlement
            .queue_construction(
                ProjectId(1),
                Position { x: 0, y: 0 },
                definition(vec![]),
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, amount)]),
                    turn_cost: 1,
                },
                &test_map(),
                &mut territory,
            )
            .unwrap();
        settlement
            .inventory_manager
            .main
            .add(ResourceType::Wood, amount);

        settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);

        assert_eq!(stock(&settlement, ResourceType::Wood), 0);
        assert_eq!(settlement.building_manager.buildings.len(), 1);
    }

    #[test]
    fn production_is_available_for_consumption_in_the_next_turn() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 2, Arc::default());
        add_building(
            &mut settlement,
            1,
            definition(vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(ResourceType::Grain, 1)],
            }]),
            &mut territory,
        );
        add_building(
            &mut settlement,
            2,
            definition(vec![recipe(ResourceType::Grain, ResourceType::Stone)]),
            &mut territory,
        );

        settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
        assert_eq!(stock(&settlement, ResourceType::Grain), 1);
        assert_eq!(stock(&settlement, ResourceType::Stone), 0);

        settlement.process_turn(&test_map(), &mut test_finder(), &mut territory);
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
