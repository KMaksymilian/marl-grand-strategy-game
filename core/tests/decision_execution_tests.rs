//! Agent decisions executed through `Simulation::step`: effects on the world,
//! untouched state for rejected actions, and reported results.
use simulation_core::buildings::Building;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::economy::ResourceType::{self, Grain, Labour, Stone, Wood};
use simulation_core::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use simulation_core::settlement::Settlement;
use simulation_core::settlement::command::SettlementCommand;
use simulation_core::settlement::decision::{OfferSide, OfferSpec};
use simulation_core::settlement::projects::{ProjectCost, ProjectResult};
use simulation_core::simulation::{
    ActionEffect, ActionError, ActionOutcome, SettlementAction, SettlementDecision, Simulation,
    SimulationTurnReport,
};
use simulation_core::trade::offer::TradeOffer;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::territory::{ProjectReference, TileOccupancy};
use simulation_core::world::{Map, Position, Province, World, terrain::area::Area};
use std::collections::HashMap;
use std::sync::Arc;

const ME: SettlementId = SettlementId(1);
const NEIGHBOUR: SettlementId = SettlementId(2);
const FARM: BuildingId = BuildingId(1);

fn at(x: i32, y: i32) -> Position {
    Position { x, y }
}

fn farm_definition(id: &str, max_workers: u32, wood: u32, turns: u32) -> BuildingDefinition {
    BuildingDefinition::new(
        id,
        id,
        1,
        max_workers,
        vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(Grain, 2)],
        }],
        ProjectCost {
            needed_resources: HashMap::from([(Wood, wood)]),
            turn_cost: turns,
        },
    )
    .unwrap()
}

/// Provinces: `10 10 20 30 / 10 ~10 20 30` (`~` is ocean). Settlement 1 owns 10
/// (population 5, Wood 10, Grain 3, gold 20, an unstaffed farm at (0,0)),
/// settlement 2 owns 20, province 30 is empty.
fn world() -> World {
    let mut terrain = vec![Area::new(0.5, 0.5); 8];
    terrain[5] = Area::new(0.0, 0.5);
    let mut world = World::new(
        Map::new(4, 2, terrain, vec![10, 10, 20, 30, 10, 10, 20, 30]),
        vec![Province::new(10), Province::new(20), Province::new(30)],
    );
    let mut catalog = BuildingCatalog::default();
    catalog.register(farm_definition("farm", 4, 2, 3));
    catalog.register(farm_definition("big_farm", 6, 5, 4));
    world.set_building_catalog(catalog);
    let mut me = Settlement::new(ME, 100, 5, Arc::default());
    me.deposit(Wood, 10);
    me.deposit(Grain, 3);
    me.deposit_gold(20);
    world.add_settlement(10, me).unwrap();
    world
        .add_settlement(20, Settlement::new(NEIGHBOUR, 100, 3, Arc::default()))
        .unwrap();
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    world
        .add_building(
            ME,
            Building::new(FARM, at(0, 0), farm),
            &mut PathFinder::new(4, 2),
        )
        .unwrap();
    world
}

fn me(simulation: &Simulation) -> &Settlement {
    simulation.settlement(ME).unwrap()
}

fn farm(simulation: &Simulation) -> &Building {
    me(simulation)
        .buildings()
        .find(|building| building.instance_id() == FARM)
        .unwrap()
}

fn step_with(simulation: &mut Simulation, actions: Vec<SettlementAction>) -> SimulationTurnReport {
    simulation
        .queue_decision(SettlementDecision::new(ME, actions))
        .unwrap();
    simulation.step().unwrap()
}

fn outcomes(report: &SimulationTurnReport) -> Vec<ActionOutcome> {
    report.decisions[0]
        .results
        .iter()
        .map(|result| result.outcome.clone())
        .collect()
}

fn applied(effect: ActionEffect) -> ActionOutcome {
    ActionOutcome::Applied { effect }
}

fn rejected(error: ActionError) -> ActionOutcome {
    ActionOutcome::Rejected { error }
}

fn construct(building_type: &str, position: Position) -> SettlementAction {
    SettlementAction::Construct {
        building_type: BuildingTypeId::new(building_type),
        position,
    }
}

fn workers(workers: u32) -> SettlementAction {
    SettlementAction::SetBuildingWorkers {
        building_id: FARM,
        workers,
    }
}

fn offer(resource: ResourceType, side: OfferSide, units: u32, price: u32) -> OfferSpec {
    OfferSpec {
        resource,
        side,
        units,
        price_per_unit: price,
    }
}

#[test]
fn every_action_reaches_the_matching_settlement_operation() {
    let mut simulation = Simulation::new(world()).unwrap();
    let report = step_with(
        &mut simulation,
        vec![
            construct("farm", at(1, 0)),
            SettlementAction::Upgrade {
                building_id: FARM,
                building_type: BuildingTypeId::new("big_farm"),
            },
            workers(4),
            SettlementAction::SetStorageOrder {
                order: vec![Grain, Stone, Wood],
            },
            SettlementAction::PublishMarketOffers {
                offers: vec![offer(Wood, OfferSide::Sell, 3, 2)],
            },
            SettlementAction::OrderCaravan {
                receiver_id: NEIGHBOUR,
                offers: vec![offer(Wood, OfferSide::Sell, 2, 1)],
            },
        ],
    );
    assert_eq!(
        outcomes(&report),
        vec![
            applied(ActionEffect::ProjectQueued {
                project_id: ProjectId(1)
            }),
            applied(ActionEffect::ProjectQueued {
                project_id: ProjectId(2)
            }),
            applied(ActionEffect::WorkersSet {
                building_id: FARM,
                workers: 4
            }),
            applied(ActionEffect::StorageOrderSet),
            applied(ActionEffect::MarketOffersPublished),
            applied(ActionEffect::CaravanOrdered {
                receiver_id: NEIGHBOUR
            }),
        ]
    );

    // Construction: catalog cost, reserved tile, one turn of progress.
    let projects = me(&simulation).active_projects();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].id(), ProjectId(1));
    assert!(matches!(
        projects[0].result(),
        ProjectResult::ConstructBuilding { position, definition }
            if *position == at(1, 0) && definition.id() == "farm"
    ));
    assert_eq!(projects[0].remaining_cost().turn_cost, 2);
    assert_eq!(
        simulation.world().tile_at(at(1, 0)).unwrap().occupancy(),
        TileOccupancy::Construction(ProjectReference {
            settlement_id: ME,
            project_id: ProjectId(1)
        })
    );
    // Upgrade: queued for the existing farm with the catalog's time cost.
    assert!(matches!(
        projects[1].result(),
        ProjectResult::UpgradeBuilding { target_instance_id, new_definition }
            if *target_instance_id == FARM && new_definition.id() == "big_farm"
    ));
    assert_eq!(projects[1].remaining_cost().turn_cost, 3);
    // Workers: applied before allocation, so the farm produced this turn.
    assert_eq!(
        Building::workers_at_load(4, farm(&simulation).target_load()),
        4
    );
    let local = &report.settlements[&ME];
    assert_eq!(local.inventory.produced[&Grain], 2);
    // Inventory policy.
    assert_eq!(me(&simulation).storage_order(), &[Grain, Stone, Wood]);
    // Market offers stay published for later turns.
    assert_eq!(
        me(&simulation).market_offers(),
        &HashMap::from([(Wood, TradeOffer::sell(3, 2))])
    );
    assert_eq!(local.trade.market_reserved[&Wood], 3);
    // The caravan was funded from stock in the same turn.
    assert_eq!(local.trade.caravans_dispatched.len(), 1);
    assert_eq!(local.trade.caravans_dispatched[0].receiver_id, NEIGHBOUR);
    assert_eq!(local.trade.caravans_dispatched[0].cargo[&Wood], 2);
    assert!(local.inventory.is_balanced());
}

#[test]
fn cancel_and_clear_undo_earlier_decisions() {
    let mut simulation = Simulation::new(world()).unwrap();
    step_with(
        &mut simulation,
        vec![
            construct("farm", at(1, 0)),
            SettlementAction::PublishMarketOffers {
                offers: vec![offer(Grain, OfferSide::Buy, 2, 1)],
            },
        ],
    );
    assert_eq!(me(&simulation).active_projects().len(), 1);
    let report = step_with(
        &mut simulation,
        vec![
            SettlementAction::CancelProject {
                project_id: ProjectId(1),
            },
            SettlementAction::ClearMarketOffers,
        ],
    );
    assert_eq!(
        outcomes(&report),
        vec![
            applied(ActionEffect::ProjectCanceled {
                project_id: ProjectId(1)
            }),
            applied(ActionEffect::MarketOffersCleared),
        ]
    );
    assert!(me(&simulation).active_projects().is_empty());
    assert!(me(&simulation).market_offers().is_empty());
    assert!(report.settlements[&ME].trade.market_reserved.is_empty());
    assert_eq!(
        simulation.world().tile_at(at(1, 0)).unwrap().occupancy(),
        TileOccupancy::Empty
    );
    // Canceled IDs are not given out again.
    let report = step_with(&mut simulation, vec![construct("farm", at(1, 0))]);
    assert_eq!(
        outcomes(&report),
        vec![applied(ActionEffect::ProjectQueued {
            project_id: ProjectId(2)
        })]
    );
}

#[test]
fn staffing_switches_production_on_and_off() {
    let mut world = world();
    world
        .execute_settlement_command(
            ME,
            &SettlementCommand::SetBuildingActive {
                building_id: FARM,
                active: false,
            },
        )
        .unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    // Staffing a switched-off building also switches it on.
    let report = step_with(&mut simulation, vec![workers(4)]);
    assert!(farm(&simulation).is_active());
    assert_eq!(report.settlements[&ME].inventory.produced[&Grain], 2);
    // Half the staff produces half the output.
    let report = step_with(&mut simulation, vec![workers(2)]);
    assert_eq!(report.settlements[&ME].inventory.produced[&Grain], 1);
    // Zero workers stops production but leaves the building in place.
    let report = step_with(&mut simulation, vec![workers(0)]);
    assert!(report.settlements[&ME].inventory.produced.is_empty());
    assert_eq!(me(&simulation).buildings().count(), 1);
}

#[test]
fn rejected_actions_change_nothing_while_valid_ones_apply() {
    let mut simulation = Simulation::new(world()).unwrap();
    let report = step_with(
        &mut simulation,
        vec![
            construct("farm", at(1, 1)),
            workers(9),
            SettlementAction::PublishMarketOffers {
                offers: vec![offer(Labour, OfferSide::Sell, 1, 1)],
            },
            SettlementAction::SetStorageOrder {
                order: vec![Wood, Wood, Grain],
            },
            SettlementAction::CancelProject {
                project_id: ProjectId(77),
            },
            SettlementAction::OrderCaravan {
                receiver_id: NEIGHBOUR,
                offers: vec![offer(Wood, OfferSide::Sell, 50, 1)],
            },
            construct("farm", at(1, 0)),
        ],
    );
    let results = &report.decisions[0].results;
    assert_eq!(
        results
            .iter()
            .map(|result| result.index)
            .collect::<Vec<_>>(),
        (0..7).collect::<Vec<_>>()
    );
    assert!(results[..6].iter().all(|result| !result.is_applied()));
    assert_eq!(
        results[6].outcome,
        applied(ActionEffect::ProjectQueued {
            project_id: ProjectId(1)
        })
    );
    assert_eq!(
        simulation.world().tile_at(at(1, 1)).unwrap().occupancy(),
        TileOccupancy::Empty
    );
    assert_eq!(farm(&simulation).target_load(), 0.0);
    assert!(me(&simulation).market_offers().is_empty());
    assert_eq!(me(&simulation).storage_order(), &[Wood, Stone, Grain]);
    assert_eq!(me(&simulation).active_projects().len(), 1);
    assert!(report.settlements[&ME].trade.caravans_dispatched.is_empty());
    assert!(me(&simulation).caravan_orders().is_empty());
}

#[test]
fn caravans_are_checked_against_what_the_market_leaves() {
    let mut simulation = Simulation::new(world()).unwrap();
    let sell_wood = |units| offer(Wood, OfferSide::Sell, units, 1);
    // Stock 10: the market, funded first, takes all of it, so nothing is left.
    let report = step_with(
        &mut simulation,
        vec![
            SettlementAction::PublishMarketOffers {
                offers: vec![sell_wood(10)],
            },
            SettlementAction::OrderCaravan {
                receiver_id: NEIGHBOUR,
                offers: vec![sell_wood(5)],
            },
        ],
    );
    assert_eq!(
        outcomes(&report)[1],
        rejected(ActionError::InsufficientStock {
            resource: Wood,
            requested: 5,
            available: 0
        })
    );
    assert!(
        report.settlements[&ME]
            .trade
            .unfunded_caravan_orders
            .is_empty()
    );

    // The published offer persists: a caravan in a later turn sees the same limit.
    let report = step_with(
        &mut simulation,
        vec![SettlementAction::OrderCaravan {
            receiver_id: NEIGHBOUR,
            offers: vec![sell_wood(1)],
        }],
    );
    assert_eq!(
        report.decisions[0].results[0].outcome,
        ActionOutcome::Rejected {
            error: ActionError::InsufficientStock {
                resource: Wood,
                requested: 1,
                available: 0,
            },
        },
    );

    // Caravan first: a later market action may not take what it was promised.
    let report = step_with(
        &mut simulation,
        vec![
            SettlementAction::ClearMarketOffers,
            SettlementAction::OrderCaravan {
                receiver_id: NEIGHBOUR,
                offers: vec![sell_wood(4)],
            },
        ],
    );
    assert!(
        report.decisions[0]
            .results
            .iter()
            .all(|result| result.is_applied())
    );
    let local = &report.settlements[&ME].trade;
    assert_eq!(local.caravans_dispatched[0].cargo[&Wood], 4);
    let last = simulation.observe(ME).unwrap().last_turn.unwrap();
    assert_eq!(last.caravans_dispatched, 1);
    assert_eq!(last.unfunded_caravan_orders, 0);

    let stock = simulation.settlement(ME).unwrap().inventory().amount(Wood);
    let report = step_with(
        &mut simulation,
        vec![
            SettlementAction::OrderCaravan {
                receiver_id: NEIGHBOUR,
                offers: vec![sell_wood(2)],
            },
            SettlementAction::PublishMarketOffers {
                offers: vec![sell_wood(stock)],
            },
        ],
    );
    assert_eq!(
        outcomes(&report)[1],
        rejected(ActionError::InsufficientStock {
            resource: Wood,
            requested: u64::from(stock),
            available: stock - 2
        })
    );
    assert_eq!(
        report.settlements[&ME].trade.caravans_dispatched[0].cargo[&Wood],
        2
    );
}

#[test]
fn conflicting_actions_apply_only_the_first() {
    let mut simulation = Simulation::new(world()).unwrap();
    let report = step_with(&mut simulation, vec![workers(4), workers(1)]);
    assert_eq!(
        outcomes(&report),
        vec![
            applied(ActionEffect::WorkersSet {
                building_id: FARM,
                workers: 4
            }),
            rejected(ActionError::Conflict { with_index: 0 }),
        ]
    );
    assert_eq!(
        Building::workers_at_load(4, farm(&simulation).target_load()),
        4
    );
}

#[test]
fn decisions_run_in_settlement_id_order_with_local_project_ids() {
    let mut simulation = Simulation::new(world()).unwrap();
    simulation
        .queue_decision(SettlementDecision::new(
            NEIGHBOUR,
            vec![SettlementAction::Construct {
                building_type: BuildingTypeId::new("farm"),
                position: at(2, 1),
            }],
        ))
        .unwrap();
    simulation
        .queue_decision(SettlementDecision::new(ME, vec![workers(1)]))
        .unwrap();
    let report = simulation.step().unwrap();
    let order: Vec<_> = report
        .decisions
        .iter()
        .map(|decision| decision.settlement_id)
        .collect();
    assert_eq!(order, vec![ME, NEIGHBOUR]);
    // Project IDs are local: both settlements start at 1.
    assert_eq!(
        report.decisions[1].results[0].outcome,
        applied(ActionEffect::ProjectQueued {
            project_id: ProjectId(1)
        })
    );
}

#[test]
fn json_from_an_agent_runs_and_results_go_back_as_json() {
    let mut simulation = Simulation::new(world()).unwrap();
    let decision = SettlementDecision::from_json(
        r#"{"settlement_id": 1, "actions": [
            {"type": "set_building_workers", "building_id": 1, "workers": 2},
            {"type": "construct", "building_type": "farm", "position": {"x": 3, "y": 0}}
        ]}"#,
    )
    .unwrap();
    simulation.queue_decision(decision).unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        serde_json::to_value(&report.decisions).unwrap(),
        serde_json::json!([{
            "settlement_id": 1,
            "results": [
                {"index": 0, "kind": "set_building_workers",
                 "outcome": {"status": "applied",
                             "effect": {"effect": "workers_set", "building_id": 1, "workers": 2}}},
                {"index": 1, "kind": "construct",
                 "outcome": {"status": "rejected",
                             "error": {"reason": "outside_own_province", "position": {"x": 3, "y": 0}}}}
            ]
        }])
    );
}
