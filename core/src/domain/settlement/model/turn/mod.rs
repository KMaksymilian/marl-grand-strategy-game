//! Settlement phases shared by local turns and the global coordinator.
pub mod report;
use super::Settlement;
use super::economy::BuildingRequests;
use super::projects::ProjectOutcome;
use super::turn_report::TradeTurnReport;
use super::turn_report::{ConsumptionResult, InventoryTurnReport, MaterialTotals, TurnReport};
use crate::domain::buildings::productivity::TravelCostSource;
use crate::domain::economy::ResourceType;
use crate::domain::economy::allocation::{AllocationEngine, AllocationRecipient, AllocationResult};
use crate::domain::ids::BuildingId;
use crate::domain::nation::taxation::tax_amount;
use crate::domain::spatial::ConstructionSpace;
use crate::domain::spatial::site::SiteEvaluation;
use crate::domain::trade::caravan::Caravan;
use std::collections::HashMap;

pub(crate) struct PreparedTurn {
    trade: TradeTurnReport,
    projects: Vec<ProjectOutcome>,
    opening_stock: MaterialTotals,
    retained_stock_limit: Option<u32>,
    building_requests: BuildingRequests,
}

pub(crate) struct AllocatedTurn {
    trade: TradeTurnReport,
    projects: Vec<ProjectOutcome>,
    allocations: AllocationResult,
    sites: HashMap<BuildingId, SiteEvaluation>,
    cycle_recipes: HashMap<BuildingId, usize>,
    retained_stock_limit: Option<u32>,
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

impl ProducedTurn {
    /// Takes the production tax out of this turn's gross production, before it
    /// enters the buffer and the warehouse, and records it in the inventory
    /// report. Each material is taxed separately and rounded down.
    pub(crate) fn levy_production_tax(&mut self, rate: f32) -> MaterialTotals {
        let mut taxed = MaterialTotals::new();
        for (resource, amount) in &mut self.produced {
            if resource == &ResourceType::Labour {
                continue;
            }
            let tax = tax_amount(rate, *amount);
            if tax > 0 {
                *amount -= tax;
                taxed.insert(resource.clone(), tax);
            }
        }
        self.consumed.allocated.inventory.taxed = taxed.clone();
        taxed
    }
}

impl Settlement {
    /// Executes the settlement phases in isolation for tests.
    #[cfg(test)]
    pub(crate) fn process_turn_for_test(
        &mut self,
        environment: &mut (impl ConstructionSpace + TravelCostSource),
        visitors: &mut [&mut Caravan],
    ) -> TurnReport {
        let prepared = self.prepare_turn(environment, visitors, 1, 0);
        let allocated = self.allocate_turn(prepared);
        let consumed = self.consume_turn(allocated);
        self.progress_construction(&consumed);
        let produced = self.produce_turn(consumed);
        self.finish_turn(produced, environment)
    }

    pub(crate) fn prepare_turn(
        &mut self,
        environment: &mut (impl ConstructionSpace + TravelCostSource),
        visitors: &mut [&mut Caravan],
        turn: u64,
        province_id: usize,
    ) -> PreparedTurn {
        self.building_manager.refresh_productivity(environment);
        let main = &self.inventory_manager.main;
        let retained_stock_limit = (main.total_amount() > u64::from(main.capacity()))
            .then(|| u32::try_from(main.total_amount()).expect("stock fit the previous capacity"));
        let mut opening_stock =
            InventoryTurnReport::snapshot(main, &self.inventory_manager.pending_production);
        for (resource, amount) in self.trade.escrowed_goods() {
            *opening_stock.entry(resource.clone()).or_default() += u64::from(amount);
        }
        let projects = self.reject_invalid_projects(environment);
        self.inventory_manager
            .refresh_labor_capacity(self.population_manager.population);
        let mut trade = TradeTurnReport::default();
        self.reserve_market(&mut trade);
        self.trade_with_visitors(visitors, &mut trade, turn, province_id);
        let building_requests = self.building_manager.generate_requests(
            |building| {
                environment
                    .evaluate_site(
                        building.position(),
                        building.definition().site_requirements(),
                    )
                    .ok()
            },
            |resource| self.inventory_manager.main.amount(resource.clone()),
        );
        PreparedTurn {
            trade,
            projects,
            opening_stock,
            retained_stock_limit,
            building_requests,
        }
    }

    pub(crate) fn allocate_turn(&mut self, prepared: PreparedTurn) -> AllocatedTurn {
        let mut requests = Vec::new();
        requests.extend(self.population_manager.generate_requests());
        requests.extend(self.project_manager.generate_requests());
        requests.extend(prepared.building_requests.demands);
        let allocations = AllocationEngine::execute(&mut self.inventory_manager.main, requests);
        let inventory = InventoryTurnReport::from_allocations(prepared.opening_stock, &allocations);
        AllocatedTurn {
            trade: prepared.trade,
            projects: prepared.projects,
            allocations,
            sites: prepared.building_requests.sites,
            cycle_recipes: prepared.building_requests.recipes,
            retained_stock_limit: prepared.retained_stock_limit,
            inventory,
        }
    }

    pub(crate) fn consume_turn(&mut self, mut allocated: AllocatedTurn) -> ConsumedTurn {
        let (consumption, substitutions) = self.population_manager.consume_turn(
            allocated.allocations.get(&AllocationRecipient::Population),
            &mut self.inventory_manager.main,
        );
        for (resource, units) in substitutions {
            *allocated
                .inventory
                .population_consumed
                .entry(resource)
                .or_default() += u64::from(units);
        }
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
        let (produced, returned) = self.building_manager.process_allocations(
            &consumed.allocated.allocations,
            &consumed.allocated.sites,
            &consumed.allocated.cycle_recipes,
        );
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
        environment: &mut (impl ConstructionSpace + TravelCostSource),
    ) -> TurnReport {
        let ProducedTurn {
            consumed,
            produced,
            mut returned,
        } = produced;
        let ConsumedTurn {
            allocated,
            consumption,
        } = consumed;
        let AllocatedTurn {
            mut trade,
            mut projects,
            mut inventory,
            retained_stock_limit,
            ..
        } = allocated;
        self.realize_caravan_orders(&mut trade);
        let mut incoming = produced;
        let mut trade_returns = self.collect_trade_returns(&mut trade);
        if let Some(limit) = retained_stock_limit {
            for (resource, &units) in &trade.market_unsold {
                *trade_returns
                    .get_mut(resource)
                    .expect("unsold stock was returned") -= units;
                *returned.entry(resource.clone()).or_default() += units;
            }
            inventory.losses.extend(
                self.inventory_manager
                    .restore_reserved_stock(std::mem::take(&mut returned), limit),
            );
        }
        for (resource, amount) in trade_returns {
            *incoming.entry(resource).or_default() += amount;
        }
        inventory.trade_reserved = trade.reserved();
        inventory.trade_received = trade.received.clone();
        inventory.losses.extend(
            self.inventory_manager
                .process_production_results(incoming, returned),
        );
        inventory
            .losses
            .extend(self.inventory_manager.commit_pending_production());
        inventory.closing_stock = InventoryTurnReport::snapshot(
            &self.inventory_manager.main,
            &self.inventory_manager.pending_production,
        );
        projects.extend(self.resolve_completed_projects(environment));
        TurnReport {
            trade,
            consumption,
            projects,
            inventory,
            taxation: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::world::settlement_environment::{
        MapConstructionSpace, MapSettlementEnvironment,
    };
    use crate::domain::world::territory::TerritoryManager;
    use crate::test_support::material;

    use crate::domain::ids::SettlementId;
    use crate::domain::world::map as core_map;
    use crate::domain::world::pathfinding::path_finder as test_path;
    use crate::domain::world::terrain::area as test_area;

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
    use crate::domain::buildings::Building;
    use crate::domain::buildings::definition::BuildingBehavior;
    use crate::domain::buildings::definition::BuildingDefinition;
    use crate::domain::economy::ResourceType;
    use crate::domain::ids::{BuildingId, ProjectId};
    use crate::domain::settlement::NeedRegistry;
    use crate::domain::settlement::NeedTier;
    use crate::domain::settlement::projects::{Project, ProjectCost};
    use crate::domain::settlement::projects::{
        ProjectCompletion, ProjectError, ProjectOutcome, ProjectResult,
    };
    use crate::domain::settlement::turn_report::{InventoryLoss, InventoryLossStage};
    use crate::domain::spatial::Position;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn definition(behaviors: Vec<BuildingBehavior>) -> Arc<BuildingDefinition> {
        Arc::new(
            BuildingDefinition::new("test", "Test", 1, 1, behaviors, ProjectCost::default())
                .unwrap(),
        )
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
        );
        building.set_target_load(1.0).unwrap();
        settlement
            .add_building(
                building,
                &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), territory),
            )
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
            .set_storage_order(
                [
                    &[material("grain"), material("wood"), material("stone")],
                    &crate::domain::economy::ResourceCatalog::standard().materials()[3..],
                ]
                .concat(),
            )
            .unwrap();
        wood_first
            .set_storage_order(
                [
                    &[material("wood"), material("stone"), material("grain")],
                    &crate::domain::economy::ResourceCatalog::standard().materials()[3..],
                ]
                .concat(),
            )
            .unwrap();
        let blueprint = definition(vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(material("wood"), 3), (material("grain"), 3)],
        }]);
        add_building(&mut food_first, 1, blueprint.clone(), &mut territory);
        add_building(&mut wood_first, 1, blueprint, &mut territory);
        let food_report = food_first.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        let wood_report = wood_first.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(food_first.storage_order()[0], material("grain"));
        assert_eq!(wood_first.storage_order()[0], material("wood"));
        assert_eq!(stock(&food_first, material("grain")), 3);
        assert_eq!(stock(&food_first, material("wood")), 0);
        assert_eq!(stock(&wood_first, material("wood")), 3);
        assert_eq!(stock(&wood_first, material("grain")), 0);
        assert!(food_report.inventory.is_balanced());
        assert!(wood_report.inventory.is_balanced());
    }

    #[test]
    fn changing_storage_priority_affects_future_admissions_without_eviction() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 3, 1, Arc::default());
        settlement
            .set_storage_order(
                [
                    &[material("grain"), material("wood"), material("stone")],
                    &crate::domain::economy::ResourceCatalog::standard().materials()[3..],
                ]
                .concat(),
            )
            .unwrap();
        add_building(
            &mut settlement,
            1,
            definition(vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(material("wood"), 3), (material("grain"), 3)],
            }]),
            &mut territory,
        );
        assert!(
            settlement
                .process_turn_for_test(
                    &mut MapSettlementEnvironment::new(
                        &test_map(),
                        &mut test_finder(),
                        &mut territory
                    ),
                    &mut []
                )
                .inventory
                .is_balanced()
        );
        assert_eq!(stock(&settlement, material("grain")), 3);

        settlement
            .set_storage_order(
                [
                    &[material("wood"), material("stone"), material("grain")],
                    &crate::domain::economy::ResourceCatalog::standard().materials()[3..],
                ]
                .concat(),
            )
            .unwrap();
        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(stock(&settlement, material("grain")), 3);
        assert_eq!(stock(&settlement, material("wood")), 0);
        assert!(report.inventory.is_balanced());

        assert!(
            settlement
                .inventory_manager
                .main
                .consume(material("grain"), 3)
        );
        assert!(
            settlement
                .process_turn_for_test(
                    &mut MapSettlementEnvironment::new(
                        &test_map(),
                        &mut test_finder(),
                        &mut territory
                    ),
                    &mut []
                )
                .inventory
                .is_balanced()
        );
        assert_eq!(stock(&settlement, material("wood")), 3);
        assert_eq!(stock(&settlement, material("grain")), 0);
    }

    #[test]
    fn buffer_and_warehouse_losses_are_reported_without_double_counting() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 5, 1, Arc::default());
        settlement.inventory_manager.main.add(material("stone"), 3);
        add_building(
            &mut settlement,
            1,
            definition(vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(material("wood"), 4), (material("grain"), 4)],
            }]),
            &mut territory,
        );
        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(
            report.inventory.losses,
            vec![
                InventoryLoss {
                    resource: material("grain"),
                    amount: 3,
                    stage: InventoryLossStage::ProductionBuffer
                },
                InventoryLoss {
                    resource: material("wood"),
                    amount: 2,
                    stage: InventoryLossStage::Warehouse
                },
                InventoryLoss {
                    resource: material("grain"),
                    amount: 1,
                    stage: InventoryLossStage::Warehouse
                },
            ]
        );
        assert_eq!(stock(&settlement, material("stone")), 3);
        assert_eq!(stock(&settlement, material("wood")), 2);
        assert_eq!(report.inventory.produced[&material("grain")], 4);
        assert!(report.inventory.is_balanced());
        let mut corrupted = report.inventory;
        corrupted.closing_stock.insert(material("wood"), 3);
        assert!(!corrupted.is_balanced());
    }

    #[test]
    fn unsold_trade_goods_compete_with_production_by_storage_order() {
        use crate::domain::trade::offer::TradeOffer;
        let mut territory = TerritoryManager::default();

        let mut settlement = Settlement::new(SettlementId(1), 10, 1, Arc::default());
        settlement
            .set_storage_order(
                [
                    &[material("grain"), material("wood"), material("stone")],
                    &crate::domain::economy::ResourceCatalog::standard().materials()[3..],
                ]
                .concat(),
            )
            .unwrap();
        settlement.inventory_manager.main.add(material("grain"), 10);
        settlement
            .publish_trade_offers(HashMap::from([(
                material("grain"),
                TradeOffer::sell(10, 1),
            )]))
            .unwrap();
        add_building(
            &mut settlement,
            1,
            definition(vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(material("wood"), 10)],
            }]),
            &mut territory,
        );

        // Nobody buys the grain, so it comes back while the warehouse receives new wood.
        let report = settlement.process_turn_for_test(
            &mut crate::domain::world::settlement_environment::MapSettlementEnvironment::new(
                &test_map(),
                &mut test_finder(),
                &mut crate::domain::world::territory::TerritoryManager::default(),
            ),
            &mut [],
        );
        assert_eq!(report.inventory.trade_received[&material("grain")], 10);
        assert_eq!(report.inventory.produced[&material("wood")], 10);
        assert_eq!(stock(&settlement, material("grain")), 10);
        assert_eq!(stock(&settlement, material("wood")), 0);
        assert_eq!(
            report.inventory.losses,
            vec![InventoryLoss {
                resource: material("wood"),
                amount: 10,
                stage: InventoryLossStage::ProductionBuffer,
            }]
        );
        assert!(report.inventory.is_balanced());
    }

    #[test]
    fn material_grants_returned_without_workers_are_not_consumption() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 2, 0, Arc::default());
        settlement.inventory_manager.main.add(material("wood"), 2);
        add_building(
            &mut settlement,
            1,
            definition(vec![BuildingBehavior::Production {
                inputs: vec![(material("wood"), 2)],
                outputs: vec![(material("stone"), 5)],
            }]),
            &mut territory,
        );
        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(report.inventory.returned[&material("wood")], 2);
        assert_eq!(report.inventory.production_consumed[&material("wood")], 0);
        assert!(report.inventory.produced.is_empty());
        assert!(report.inventory.losses.is_empty());
        assert_eq!(stock(&settlement, material("wood")), 2);
        assert!(report.inventory.is_balanced());
    }

    #[test]
    fn combined_building_production_can_exceed_u32_without_wrapping() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), u32::MAX, 2, Arc::default());
        let blueprint = definition(vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(material("wood"), u32::MAX)],
        }]);
        add_building(&mut settlement, 1, blueprint.clone(), &mut territory);
        add_building(&mut settlement, 2, blueprint, &mut territory);
        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(
            report.inventory.produced[&material("wood")],
            2 * u64::from(u32::MAX)
        );
        assert_eq!(
            report.inventory.losses,
            vec![InventoryLoss {
                resource: material("wood"),
                amount: u64::from(u32::MAX),
                stage: InventoryLossStage::ProductionBuffer,
            }]
        );
        assert_eq!(stock(&settlement, material("wood")), u32::MAX);
        assert!(report.inventory.is_balanced());
    }

    #[test]
    fn opening_balance_includes_preexisting_buffer_stock() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 3, 0, Arc::default());
        settlement.inventory_manager.main.add(material("grain"), 1);
        settlement
            .inventory_manager
            .pending_production
            .add(material("wood"), 3);
        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(report.inventory.opening_stock[&material("wood")], 3);
        assert_eq!(report.inventory.closing_stock[&material("wood")], 2);
        assert_eq!(
            report.inventory.losses,
            vec![InventoryLoss {
                resource: material("wood"),
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
                    material("stone")
                } else {
                    material("wood")
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
            let report = settlement.process_turn_for_test(
                &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
                &mut [],
            );
            assert!(report.inventory.is_balanced());
            if let Some(expected) = &expected {
                assert_eq!(&report, expected);
            } else {
                expected = Some(report);
            }
            assert_eq!(stock(&settlement, material("wood")), 3);
            assert_eq!(stock(&settlement, material("stone")), 0);
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
                    needed_resources: HashMap::from([(material("wood"), 0)]),
                    turn_cost: 0,
                },
                &mut MapConstructionSpace::new(&test_map(), &mut territory),
            )
            .unwrap();
        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
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
                .process_turn_for_test(
                    &mut MapSettlementEnvironment::new(
                        &test_map(),
                        &mut test_finder(),
                        &mut territory
                    ),
                    &mut []
                )
                .projects
                .is_empty()
        );
    }

    #[test]
    fn missing_upgrade_target_is_rejected_without_spending_resources() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
        settlement.inventory_manager.main.add(material("wood"), 10);
        let result = settlement.queue_upgrade(
            ProjectId(10),
            BuildingId(99),
            definition(vec![]),
            ProjectCost {
                needed_resources: HashMap::from([(material("wood"), 5)]),
                turn_cost: 1,
            },
        );
        assert_eq!(result, Err(ProjectError::MissingBuilding(BuildingId(99))));
        assert!(settlement.project_manager.active_projects.is_empty());
        settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(stock(&settlement, material("wood")), 10);
    }

    #[test]
    fn upgrade_completion_reports_the_existing_building_id() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
        let old = definition(vec![]);
        let upgraded = definition(vec![]);
        settlement
            .add_building(
                Building::new(BuildingId(8), Position { x: 2, y: 3 }, old),
                &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            )
            .unwrap();
        settlement
            .queue_upgrade(
                ProjectId(4),
                BuildingId(8),
                upgraded.clone(),
                ProjectCost {
                    needed_resources: HashMap::from([
                        (material("wood"), 2),
                        (material("stone"), 0),
                    ]),
                    turn_cost: 1,
                },
            )
            .unwrap();
        settlement.inventory_manager.main.add(material("wood"), 2);

        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );

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
        assert_eq!(stock(&settlement, material("wood")), 0);
        assert!(settlement.project_manager.active_projects.is_empty());
        assert!(
            settlement
                .process_turn_for_test(
                    &mut MapSettlementEnvironment::new(
                        &test_map(),
                        &mut test_finder(),
                        &mut territory
                    ),
                    &mut []
                )
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
                    needed_resources: HashMap::from([(material("wood"), 3)]),
                    turn_cost: 2,
                },
            )
            .unwrap();
        settlement.inventory_manager.main.add(material("wood"), 1);
        assert!(
            settlement
                .process_turn_for_test(
                    &mut MapSettlementEnvironment::new(
                        &test_map(),
                        &mut test_finder(),
                        &mut territory
                    ),
                    &mut []
                )
                .projects
                .is_empty()
        );
        assert_eq!(
            settlement.project_manager.active_projects[0]
                .remaining_cost
                .needed_resources[&material("wood")],
            2
        );

        settlement.building_manager.buildings.clear();
        settlement.inventory_manager.main.add(material("wood"), 2);
        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );

        assert_eq!(
            report.projects,
            vec![ProjectOutcome {
                project_id: ProjectId(5),
                result: Err(ProjectError::MissingBuilding(BuildingId(1))),
            }]
        );
        assert_eq!(stock(&settlement, material("wood")), 2);
        assert!(settlement.project_manager.active_projects.is_empty());
        assert!(
            settlement
                .process_turn_for_test(
                    &mut MapSettlementEnvironment::new(
                        &test_map(),
                        &mut test_finder(),
                        &mut territory
                    ),
                    &mut []
                )
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
                    needed_resources: HashMap::from([(material("wood"), 2)]),
                    turn_cost: 1,
                },
            ))
            .unwrap();
        settlement.inventory_manager.main.add(material("wood"), 2);
        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(
            report.projects,
            vec![ProjectOutcome {
                project_id: ProjectId(3),
                result: Err(ProjectError::UnsupportedRecruitment),
            }]
        );
        assert_eq!(stock(&settlement, material("wood")), 2);
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
                        (material("wood"), 2),
                        (material("stone"), 0),
                    ]),
                    turn_cost: 2,
                },
                &mut MapConstructionSpace::new(&test_map(), &mut territory),
            )
            .unwrap();
        settlement.inventory_manager.main.add(material("wood"), 1);
        assert!(
            settlement
                .process_turn_for_test(
                    &mut MapSettlementEnvironment::new(
                        &test_map(),
                        &mut test_finder(),
                        &mut territory
                    ),
                    &mut []
                )
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
                .process_turn_for_test(
                    &mut MapSettlementEnvironment::new(
                        &test_map(),
                        &mut test_finder(),
                        &mut territory
                    ),
                    &mut []
                )
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

        settlement.inventory_manager.main.add(material("wood"), 1);
        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(
            report.projects,
            vec![ProjectOutcome {
                project_id: ProjectId(2),
                result: Ok(ProjectCompletion::BuildingConstructed(BuildingId(0))),
            }]
        );
        assert_eq!(stock(&settlement, material("wood")), 0);
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
                &mut MapConstructionSpace::new(&test_map(), &mut territory),
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
                &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory)
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
                    needed_resources: HashMap::from([(material("wood"), 1)]),
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
                    needed_resources: HashMap::from([(material("wood"), 1)]),
                    turn_cost: 1,
                },
                &mut MapConstructionSpace::new(&test_map(), &mut territory),
            )
            .unwrap();
        settlement.inventory_manager.main.add(material("wood"), 1);

        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );

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
        assert_eq!(stock(&settlement, material("wood")), 0);
        assert_eq!(settlement.building_manager.buildings.len(), 1);
    }

    #[test]
    fn population_building_and_project_share_whole_resource_units() {
        let mut territory = TerritoryManager::default();
        let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(material("wood"), 1)]),
        }]));
        let mut settlement = Settlement::new(SettlementId(1), 100, 2, registry);
        let blueprint = definition(vec![recipe(material("wood"), material("stone"))]);
        add_building(&mut settlement, 1, blueprint.clone(), &mut territory);
        settlement
            .queue_construction(
                ProjectId(1),
                Position { x: 1, y: 0 },
                blueprint,
                ProjectCost {
                    needed_resources: HashMap::from([(material("wood"), 1)]),
                    turn_cost: 1,
                },
                &mut MapConstructionSpace::new(&test_map(), &mut territory),
            )
            .unwrap();
        settlement.inventory_manager.main.add(material("wood"), 4);

        let result = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );

        // Two residents consume 2, the recipe consumes 1, construction costs 1.
        assert_eq!(stock(&settlement, material("wood")), 0);
        assert_eq!(stock(&settlement, material("stone")), 1);
        assert_eq!(result.consumption.satisfaction_percent, 1.0);
        assert_eq!(result.inventory.population_consumed[&material("wood")], 2);
        assert_eq!(result.inventory.project_consumed[&material("wood")], 1);
        assert_eq!(result.inventory.production_consumed[&material("wood")], 1);
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
                needs_per_capita: HashMap::from([(material("grain"), 1)]),
            },
            NeedTier {
                min_population: 1,
                needs_per_capita: HashMap::from([(material("grain"), 1)]),
            },
        ]));
        let mut settlement = Settlement::new(SettlementId(1), 100, 1, registry);
        settlement.inventory_manager.main.add(material("grain"), 3);

        let result = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );

        assert_eq!(result.consumption.satisfaction_percent, 1.0);
        assert!(result.consumption.missing_resources.is_empty());
        assert_eq!(stock(&settlement, material("grain")), 1);
    }

    #[test]
    fn population_shortage_is_reported_across_turns() {
        let mut territory = TerritoryManager::default();
        let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(material("grain"), 1), (material("wood"), 1)]),
        }]));
        let mut settlement = Settlement::new(SettlementId(1), 100, 1, registry);
        settlement.inventory_manager.main.add(material("grain"), 3);

        for remaining in [2, 1] {
            let result = settlement.process_turn_for_test(
                &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
                &mut [],
            );
            assert_eq!(result.consumption.satisfaction_percent, 0.5);
            assert_eq!(result.consumption.missing_resources, vec![material("wood")]);
            assert_eq!(stock(&settlement, material("grain")), remaining);
        }
        settlement.inventory_manager.main.add(material("wood"), 1);
        let result = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(result.consumption.satisfaction_percent, 1.0);
        assert!(result.consumption.missing_resources.is_empty());
        assert_eq!(stock(&settlement, material("grain")), 0);
        assert_eq!(stock(&settlement, material("wood")), 0);
    }

    #[test]
    fn building_and_project_with_same_number_have_separate_allocations() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 1, Arc::default());
        let blueprint = definition(vec![recipe(material("wood"), material("stone"))]);
        add_building(&mut settlement, 7, blueprint.clone(), &mut territory);
        settlement
            .queue_construction(
                ProjectId(7),
                Position { x: 1, y: 0 },
                blueprint,
                ProjectCost {
                    needed_resources: HashMap::from([(material("wood"), 1)]),
                    turn_cost: 1,
                },
                &mut MapConstructionSpace::new(&test_map(), &mut territory),
            )
            .unwrap();
        settlement.inventory_manager.main.add(material("wood"), 2);

        settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );

        assert_eq!(stock(&settlement, material("wood")), 0);
        assert_eq!(stock(&settlement, material("stone")), 1);
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
            needs_per_capita: HashMap::from([(material("grain"), 1)]),
        }]));
        let mut settlement = Settlement::new(SettlementId(1), 100, 1, registry);
        add_building(
            &mut settlement,
            0,
            definition(vec![recipe(material("grain"), material("stone"))]),
            &mut territory,
        );
        settlement.inventory_manager.main.add(material("grain"), 2);

        let report = settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );

        assert_eq!(report.consumption.satisfaction_percent, 1.0);
        assert_eq!(report.inventory.population_consumed[&material("grain")], 1);
        assert!(report.inventory.project_consumed.is_empty());
        assert_eq!(report.inventory.production_consumed[&material("grain")], 1);
        assert!(report.inventory.losses.is_empty());
        assert!(report.inventory.is_balanced());
        assert_eq!(stock(&settlement, material("grain")), 0);
        assert_eq!(stock(&settlement, material("stone")), 1);
    }

    #[test]
    fn shared_recipe_inputs_are_consumed_only_once() {
        let mut territory = TerritoryManager::default();
        let mut settlement = Settlement::new(SettlementId(1), 100, 1, Arc::default());
        add_building(
            &mut settlement,
            1,
            definition(vec![
                recipe(material("wood"), material("stone")),
                recipe(material("wood"), material("grain")),
            ]),
            &mut territory,
        );
        settlement.inventory_manager.main.add(material("wood"), 1);

        settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );

        assert_eq!(stock(&settlement, material("wood")), 0);
        assert_eq!(stock(&settlement, material("stone")), 1);
        assert_eq!(stock(&settlement, material("grain")), 0);
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
                        needed_resources: HashMap::from([(material("wood"), 1)]),
                        turn_cost: 1,
                    },
                    &mut MapConstructionSpace::new(&test_map(), &mut territory),
                )
                .unwrap();
        }
        settlement.inventory_manager.main.add(material("wood"), 1);

        settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );

        assert_eq!(stock(&settlement, material("wood")), 0);
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
                    needed_resources: HashMap::from([(material("wood"), amount)]),
                    turn_cost: 1,
                },
                &mut MapConstructionSpace::new(&test_map(), &mut territory),
            )
            .unwrap();
        settlement
            .inventory_manager
            .main
            .add(material("wood"), amount);

        settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );

        assert_eq!(stock(&settlement, material("wood")), 0);
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
                outputs: vec![(material("grain"), 1)],
            }]),
            &mut territory,
        );
        add_building(
            &mut settlement,
            2,
            definition(vec![recipe(material("grain"), material("stone"))]),
            &mut territory,
        );

        settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(stock(&settlement, material("grain")), 1);
        assert_eq!(stock(&settlement, material("stone")), 0);

        settlement.process_turn_for_test(
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
            &mut [],
        );
        assert_eq!(stock(&settlement, material("grain")), 1);
        assert_eq!(stock(&settlement, material("stone")), 1);
        assert!(
            settlement
                .inventory_manager
                .pending_production
                .resources()
                .is_empty()
        );
    }
}
