use simulation_core::ids::SettlementId;
use std::collections::HashMap;
use std::sync::Arc;

use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::{
    Building,
    definition::{BuildingBehavior, BuildingDefinition},
};
use simulation_core::economy::ResourceType::{Grain, Stone, Wood};
use simulation_core::economy::storage_order::StorageOrderError;
use simulation_core::ids::BuildingTypeId;
use simulation_core::ids::{BuildingId, ProjectId};
use simulation_core::settlement::projects::{ProjectCompletion, ProjectCost};
use simulation_core::settlement::{NeedRegistry, NeedTier, Settlement};
use simulation_core::simulation::{
    ActionEffect, ActionError, ActionOutcome, SettlementAction, SettlementDecision, Simulation,
    SimulationError, TurnPhase,
};
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::area::Area;
use simulation_core::world::territory::TileOccupancy;
use simulation_core::world::{Map, Position, Province, World, WorldError};

fn definition() -> Arc<BuildingDefinition> {
    Arc::new(
        BuildingDefinition::new(
            "farm",
            "Farm",
            1,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![(Wood, 1)],
                outputs: vec![(Grain, 2)],
            }],
            ProjectCost::default(),
        )
        .unwrap(),
    )
}

fn world(order: &[u32]) -> World {
    let mut world = World::new(
        Map::new(4, 1, vec![Area::new(0.5, 0.5); 4], vec![10, 10, 20, 20]),
        vec![Province::new(10), Province::new(20)],
    );
    let mut finder = PathFinder::new(4, 1);
    for &id in order {
        let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(Grain, 1)]),
        }]));
        let mut settlement = Settlement::new(SettlementId(id), 100, 1, registry);
        settlement.deposit(Wood, 10);
        world
            .add_settlement(if id == 1 { 10 } else { 20 }, settlement)
            .unwrap();
        let mut building = Building::new(
            BuildingId(7),
            Position {
                x: if id == 1 { 0 } else { 2 },
                y: 0,
            },
            definition(),
        );
        building.set_target_load(1.0).unwrap();
        world
            .add_building(SettlementId(id), building, &mut finder)
            .unwrap();
    }
    world
}

/// The test world plus a catalog whose "farm" has the given construction cost.
fn world_with_catalog(order: &[u32], cost: ProjectCost) -> World {
    let mut world = world(order);
    let mut catalog = BuildingCatalog::default();
    catalog.register(
        BuildingDefinition::new(
            "farm",
            "Farm",
            1,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![(Wood, 1)],
                outputs: vec![(Grain, 2)],
            }],
            cost,
        )
        .unwrap(),
    );
    world.set_building_catalog(catalog);
    world
}

fn decide(simulation: &mut Simulation, actions: Vec<SettlementAction>) {
    simulation
        .queue_decision(SettlementDecision::new(SettlementId(1), actions))
        .unwrap();
}

fn workers(building_id: u32, workers: u32) -> SettlementAction {
    SettlementAction::SetBuildingWorkers {
        building_id: BuildingId(building_id),
        workers,
    }
}

fn applied(outcome: &ActionOutcome) -> &ActionEffect {
    match outcome {
        ActionOutcome::Applied { effect } => effect,
        ActionOutcome::Rejected { error } => panic!("rejected: {error:?}"),
    }
}

fn rejected(outcome: &ActionOutcome) -> &ActionError {
    match outcome {
        ActionOutcome::Rejected { error } => error,
        ActionOutcome::Applied { effect } => panic!("applied: {effect:?}"),
    }
}

#[test]
fn global_phases_finish_for_all_settlements_in_stable_id_order() {
    let mut simulation = Simulation::new(world(&[2, 1])).unwrap();
    assert_eq!(simulation.current_turn(), 0);
    let report = simulation.step().unwrap();
    let actual: Vec<_> = report
        .execution
        .iter()
        .map(|event| (event.phase, event.settlement_id))
        .collect();
    let expected: Vec<_> = [
        TurnPhase::Decisions,
        TurnPhase::Preparation,
        TurnPhase::Allocation,
        TurnPhase::Consumption,
        TurnPhase::Construction,
        TurnPhase::Production,
        TurnPhase::EconomicUpdates,
    ]
    .into_iter()
    .flat_map(|phase| [(phase, SettlementId(1)), (phase, SettlementId(2))])
    .collect();
    assert_eq!(actual, expected);
    assert_eq!(report.turn, 1);
    assert_eq!(simulation.current_turn(), 1);
    assert_eq!(report.settlements.len(), 2);
}

#[test]
fn insertion_order_does_not_change_reports_or_world_results() {
    let mut first = Simulation::new(world(&[1, 2])).unwrap();
    let mut second = Simulation::new(world(&[2, 1])).unwrap();
    for _ in 0..5 {
        let a = first.step().unwrap();
        let b = second.step().unwrap();
        assert_eq!(a.turn, b.turn);
        assert_eq!(a.execution, b.execution);
        assert_eq!(a.settlements, b.settlements);
        for id in [1, 2] {
            assert_eq!(
                first
                    .world()
                    .settlement(SettlementId(id))
                    .unwrap()
                    .inventory()
                    .amount(Grain),
                second
                    .world()
                    .settlement(SettlementId(id))
                    .unwrap()
                    .inventory()
                    .amount(Grain)
            );
        }
    }
}

#[test]
fn global_pipeline_preserves_existing_local_turn_economy() {
    let mut local = world(&[1, 2]);
    let mut simulation = Simulation::new(world(&[2, 1])).unwrap();
    let mut finder = PathFinder::new(4, 1);
    for _ in 0..4 {
        let global = simulation.step().unwrap();
        for id in [1, 2] {
            assert_eq!(
                global.settlements[&SettlementId(id)],
                local
                    .process_settlement_turn(SettlementId(id), &mut finder)
                    .unwrap()
            );
            assert!(
                global.settlements[&SettlementId(id)]
                    .inventory
                    .is_balanced()
            );
        }
    }
}

#[test]
fn production_is_available_to_consumption_on_the_next_turn() {
    let mut simulation = Simulation::new(world(&[1])).unwrap();
    let first = simulation.step().unwrap();
    assert_eq!(
        first.settlements[&SettlementId(1)]
            .consumption
            .missing_resources,
        vec![Grain]
    );
    assert_eq!(
        simulation
            .world()
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Grain),
        2
    );
    let second = simulation.step().unwrap();
    assert!(
        second.settlements[&SettlementId(1)]
            .consumption
            .missing_resources
            .is_empty()
    );
    assert_eq!(
        second.settlements[&SettlementId(1)]
            .inventory
            .population_consumed[&Grain],
        1
    );
    assert_eq!(
        simulation
            .world()
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Grain),
        3
    );
}

#[test]
fn decisions_wait_for_step_apply_in_submission_order_and_are_not_replayed() {
    let mut simulation = Simulation::new(world(&[1])).unwrap();
    decide(&mut simulation, vec![workers(7, 1)]);
    decide(&mut simulation, vec![workers(7, 0)]);
    assert_eq!(
        simulation
            .world()
            .settlement(SettlementId(1))
            .unwrap()
            .buildings()
            .next()
            .unwrap()
            .target_load(),
        1.0
    );
    let report = simulation.step().unwrap();
    assert_eq!(report.decisions.len(), 2);
    assert!(
        report
            .decisions
            .iter()
            .all(|outcome| outcome.results.iter().all(|result| result.is_applied()))
    );
    // The later decision wins, so the farm requests no workers this turn.
    assert!(
        report.settlements[&SettlementId(1)]
            .inventory
            .produced
            .is_empty()
    );
    assert_eq!(
        simulation
            .world()
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Wood),
        10
    );
    assert!(simulation.step().unwrap().decisions.is_empty());
}

#[test]
fn invalid_decisions_are_reported_without_aborting_valid_commands_or_the_turn() {
    let mut simulation = Simulation::new(world(&[1])).unwrap();
    decide(
        &mut simulation,
        vec![workers(99, 1), workers(7, 2), workers(7, 0)],
    );
    let report = simulation.step().unwrap();
    let results = &report.decisions[0].results;
    assert_eq!(
        rejected(&results[0].outcome),
        &ActionError::UnknownBuilding {
            building_id: BuildingId(99)
        }
    );
    assert_eq!(
        rejected(&results[1].outcome),
        &ActionError::WorkersAboveCapacity {
            building_id: BuildingId(7),
            workers: 2,
            max_workers: 1
        }
    );
    assert_eq!(
        applied(&results[2].outcome),
        &ActionEffect::WorkersSet {
            building_id: BuildingId(7),
            workers: 0
        }
    );
    assert_eq!(report.turn, 1);
    assert!(
        report.settlements[&SettlementId(1)]
            .inventory
            .produced
            .is_empty()
    );
}

#[test]
fn construction_progresses_once_per_global_turn_and_finishes_after_production() {
    let cost = ProjectCost {
        turn_cost: 2,
        ..ProjectCost::default()
    };
    let mut simulation = Simulation::new(world_with_catalog(&[1], cost)).unwrap();
    decide(
        &mut simulation,
        vec![
            workers(7, 0),
            SettlementAction::Construct {
                building_type: BuildingTypeId::new("farm"),
                position: Position { x: 1, y: 0 },
            },
        ],
    );
    let first = simulation.step().unwrap();
    assert_eq!(
        applied(&first.decisions[0].results[1].outcome),
        &ActionEffect::ProjectQueued {
            project_id: ProjectId(1)
        }
    );
    assert!(first.settlements[&SettlementId(1)].projects.is_empty());
    assert_eq!(
        simulation
            .world()
            .settlement(SettlementId(1))
            .unwrap()
            .active_projects()[0]
            .remaining_cost()
            .turn_cost,
        1
    );
    let second = simulation.step().unwrap();
    assert_eq!(
        second.settlements[&SettlementId(1)].projects[0].result,
        Ok(ProjectCompletion::BuildingConstructed(BuildingId(0)))
    );
    assert!(
        second.settlements[&SettlementId(1)]
            .inventory
            .produced
            .is_empty()
    );
    decide(&mut simulation, vec![workers(0, 1)]);
    let third = simulation.step().unwrap();
    assert_eq!(
        third.settlements[&SettlementId(1)].inventory.produced[&Grain],
        2
    );
    assert!(
        third.settlements[&SettlementId(1)]
            .consumption
            .missing_resources
            .contains(&Grain)
    );
}

#[test]
fn construction_can_be_canceled_in_the_same_decision_phase_without_spending() {
    let cost = ProjectCost {
        needed_resources: HashMap::from([(Wood, 4)]),
        turn_cost: 1,
    };
    let mut simulation = Simulation::new(world_with_catalog(&[1], cost)).unwrap();
    decide(
        &mut simulation,
        vec![SettlementAction::Construct {
            building_type: BuildingTypeId::new("farm"),
            position: Position { x: 1, y: 0 },
        }],
    );
    // A second decision sees the project created by the first one.
    decide(
        &mut simulation,
        vec![SettlementAction::CancelProject {
            project_id: ProjectId(1),
        }],
    );
    let report = simulation.step().unwrap();
    assert!(
        report
            .decisions
            .iter()
            .all(|decision| decision.results.iter().all(|result| result.is_applied()))
    );
    assert!(report.settlements[&SettlementId(1)].projects.is_empty());
    assert_eq!(
        simulation
            .world()
            .tile_at(Position { x: 1, y: 0 })
            .unwrap()
            .occupancy(),
        TileOccupancy::Empty
    );
    assert_eq!(
        simulation
            .world()
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Wood),
        9
    );
}

#[test]
fn registration_uses_the_world_registry_and_unknown_decisions_are_rejected() {
    let mut simulation = Simulation::new(world(&[])).unwrap();
    assert_eq!(
        simulation.queue_decision(SettlementDecision::new(
            SettlementId(1),
            vec![SettlementAction::CancelProject {
                project_id: ProjectId(1)
            }]
        )),
        Err(WorldError::MissingSettlement(SettlementId(1)))
    );
    simulation
        .add_settlement(10, Settlement::new(SettlementId(1), 100, 0, Arc::default()))
        .unwrap();
    assert_eq!(simulation.settlements().count(), 1);
    assert_eq!(
        simulation.world().province_settlement(10),
        Ok(Some(SettlementId(1)))
    );
    assert!(
        simulation
            .add_settlement(20, Settlement::new(SettlementId(1), 100, 0, Arc::default()))
            .is_err()
    );
    assert_eq!(simulation.step().unwrap().settlements.len(), 1);
}

#[test]
fn multiple_turns_run_headlessly_including_an_empty_world() {
    let mut simulation = Simulation::new(world(&[])).unwrap();
    for turn in 1..=100 {
        let report = simulation.step().unwrap();
        assert_eq!(report.turn, turn);
        assert_eq!(simulation.current_turn(), turn);
        assert!(report.settlements.is_empty());
        assert!(report.execution.is_empty());
    }
}

#[test]
fn malformed_maps_are_rejected_before_allocating_pathfinding_buffers() {
    for (width, height, terrain, territory, provinces) in [
        (0, 0, vec![], vec![], vec![]),
        (usize::MAX, 2, vec![], vec![], vec![]),
        (
            2,
            1,
            vec![Area::default()],
            vec![0, 0],
            vec![Province::new(0)],
        ),
        (
            1,
            1,
            vec![Area::default()],
            vec![99],
            vec![Province::new(0)],
        ),
        (
            1,
            1,
            vec![Area::default()],
            vec![0],
            vec![Province::new(0), Province::new(0)],
        ),
        (
            1,
            1,
            vec![Area::new(f32::NAN, 0.5)],
            vec![0],
            vec![Province::new(0)],
        ),
    ] {
        assert!(matches!(
            Simulation::new(World::new(
                Map::new(width, height, terrain, territory),
                provinces
            )),
            Err(SimulationError::InvalidMap)
        ));
    }
}

#[test]
fn storage_order_decision_is_applied_before_economic_updates() {
    let mut simulation = Simulation::new(world(&[1])).unwrap();
    decide(
        &mut simulation,
        vec![SettlementAction::SetStorageOrder {
            order: vec![Grain, Wood, Stone],
        }],
    );
    let report = simulation.step().unwrap();
    assert_eq!(
        applied(&report.decisions[0].results[0].outcome),
        &ActionEffect::StorageOrderSet
    );
    assert_eq!(
        simulation
            .world()
            .settlement(SettlementId(1))
            .unwrap()
            .storage_order(),
        &[Grain, Wood, Stone]
    );
}

#[test]
fn an_upgrade_changes_output_only_after_the_turn_in_which_it_completes() {
    let mut world = world(&[1]);
    let original = definition();
    let mut catalog = BuildingCatalog::default();
    catalog.register(
        BuildingDefinition::new(
            "big_farm",
            "Big farm",
            2,
            original.max_workers(),
            vec![BuildingBehavior::Production {
                inputs: vec![(Wood, 1)],
                outputs: vec![(Grain, 4)],
            }],
            ProjectCost::default(),
        )
        .unwrap(),
    );
    world.set_building_catalog(catalog);
    let mut simulation = Simulation::new(world).unwrap();
    decide(
        &mut simulation,
        vec![SettlementAction::Upgrade {
            building_id: BuildingId(7),
            building_type: BuildingTypeId::new("big_farm"),
        }],
    );
    let first = simulation.step().unwrap();
    assert_eq!(
        first.settlements[&SettlementId(1)].inventory.produced[&Grain],
        2
    );
    assert_eq!(
        first.settlements[&SettlementId(1)].projects[0].result,
        Ok(ProjectCompletion::BuildingUpgraded(BuildingId(7)))
    );
    assert_eq!(
        simulation.step().unwrap().settlements[&SettlementId(1)]
            .inventory
            .produced[&Grain],
        4
    );
}

#[test]
fn invalid_storage_order_is_reported_and_does_not_replace_the_existing_order() {
    let mut simulation = Simulation::new(world(&[1])).unwrap();
    let before = simulation
        .world()
        .settlement(SettlementId(1))
        .unwrap()
        .storage_order()
        .to_vec();
    decide(
        &mut simulation,
        vec![SettlementAction::SetStorageOrder {
            order: vec![Wood, Wood],
        }],
    );
    let report = simulation.step().unwrap();
    assert_eq!(
        rejected(&report.decisions[0].results[0].outcome),
        &ActionError::InvalidStorageOrder {
            error: StorageOrderError::DuplicateMaterial(Wood)
        }
    );
    assert_eq!(
        simulation
            .world()
            .settlement(SettlementId(1))
            .unwrap()
            .storage_order(),
        before
    );
}
