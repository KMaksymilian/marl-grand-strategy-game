mod common;

use common::material;
use simulation_core::buildings::{
    Building, catalog::BuildingCatalog, definition::BuildingDefinition,
};
use simulation_core::economy::{ResourceCatalog, ResourceType};
use simulation_core::ids::{BuildingId, BuildingTypeId, NationId, ProjectId, SettlementId};
use simulation_core::settlement::Settlement;
use simulation_core::settlement::decision::{OfferSide, OfferSpec};
use simulation_core::settlement::projects::ProjectCost;
use simulation_core::settlement::turn_report::CaravanDepartureRoute;
use simulation_core::simulation::{
    ActionEffect, ActionError, ActionOutcome, NationAction, NationActionEffect,
    NationActionOutcome, NationDecision, SettlementAction, SettlementDecision, Simulation,
    TurnPhase,
};
use simulation_core::trade::caravan::CaravanOutcome;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::territory::TileOccupancy;
use simulation_core::world::{Map, Position, Province, World, terrain::area::Area};
use std::{collections::BTreeMap, sync::Arc};

const SENDER: SettlementId = SettlementId(1);
const EAST: SettlementId = SettlementId(2);
const WEST: SettlementId = SettlementId(3);
const NATION: NationId = NationId(0);

fn tile(x: i32) -> Position {
    Position { x, y: 0 }
}

fn setup(disconnected: bool, same_nation: bool) -> World {
    let mut terrain = vec![Area::new(0.5, 0.5); 6];
    if disconnected {
        terrain[2] = Area::new(0.0, 0.5);
    }
    let mut world = World::new(
        Map::new(
            6,
            1,
            terrain,
            if disconnected {
                vec![1, 3, 1, 2, 2, 2]
            } else {
                vec![1, 1, 1, 2, 3, 3]
            },
        ),
        (1..=3).map(Province::new).collect(),
    );
    let mut sender = Settlement::new(SENDER, 100, 5, Arc::default());
    if !same_nation {
        sender.deposit(material("wood"), 10);
        sender.deposit_gold(20);
    }
    world.add_settlement_at(1, tile(0), sender).unwrap();
    for (province, id) in [(2, EAST), (3, WEST)] {
        let settlement = Settlement::new(id, 100, 5, Arc::default());
        if same_nation {
            world
                .add_settlement_to_nation(NATION, province, settlement)
                .unwrap();
        } else {
            world.add_settlement(province, settlement).unwrap();
        }
    }
    let mut catalog = BuildingCatalog::default();
    for name in ["farm", "large_farm"] {
        catalog.register(
            BuildingDefinition::new(
                name,
                name,
                1,
                2,
                vec![],
                ProjectCost {
                    turn_cost: 3,
                    needed_resources: Default::default(),
                },
            )
            .unwrap(),
        );
    }
    world.set_building_catalog(catalog);
    world
}

fn queue(sim: &mut Simulation, actions: Vec<SettlementAction>) {
    sim.queue_decision(SettlementDecision::new(SENDER, actions))
        .unwrap();
}

fn construct() -> SettlementAction {
    SettlementAction::Construct {
        building_type: BuildingTypeId::new("farm"),
        position: tile(1),
    }
}

#[test]
fn cancellation_releases_construction_and_upgrade_targets_in_the_same_decision() {
    let mut world = setup(false, false);
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    world
        .add_building(
            SENDER,
            Building::new(BuildingId(0), tile(0), farm),
            &mut PathFinder::new(6, 1),
        )
        .unwrap();
    let mut sim = Simulation::new(world).unwrap();
    let upgrade = || SettlementAction::Upgrade {
        building_id: BuildingId(0),
        building_type: BuildingTypeId::new("large_farm"),
    };
    queue(&mut sim, vec![construct(), upgrade()]);
    assert!(
        sim.step().unwrap().decisions[0]
            .results
            .iter()
            .all(|row| row.is_applied())
    );
    queue(
        &mut sim,
        vec![
            SettlementAction::CancelProject {
                project_id: ProjectId(1),
            },
            construct(),
            SettlementAction::CancelProject {
                project_id: ProjectId(2),
            },
            upgrade(),
        ],
    );
    let report = sim.step().unwrap();
    let actions = &report.decisions[0].results;
    assert!(actions.iter().all(|row| row.is_applied()), "{actions:?}");
    assert_eq!(
        actions.iter().map(|row| row.index).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert!(matches!(
        sim.world().tile_at(tile(1)).unwrap().occupancy(),
        TileOccupancy::Construction(reference) if reference.project_id == ProjectId(3)
    ));
    assert_eq!(sim.settlement(SENDER).unwrap().active_projects().len(), 2);
}

#[test]
fn canceling_a_project_queued_in_the_same_decision_releases_its_target() {
    let mut sim = Simulation::new(setup(false, false)).unwrap();
    queue(
        &mut sim,
        vec![
            construct(),
            SettlementAction::CancelProject {
                project_id: ProjectId(1),
            },
            construct(),
            construct(),
        ],
    );
    let report = sim.step().unwrap();
    let actions = &report.decisions[0].results;
    assert!(actions[..3].iter().all(|row| row.is_applied()));
    assert_eq!(
        actions[3].outcome,
        ActionOutcome::Rejected {
            error: ActionError::Conflict { with_index: 2 }
        }
    );
    assert!(matches!(
        sim.world().tile_at(tile(1)).unwrap().occupancy(),
        TileOccupancy::Construction(reference) if reference.project_id == ProjectId(2)
    ));
}

#[test]
fn returned_subsidy_materials_are_available_to_the_next_national_action() {
    let mut world = setup(false, true);
    world
        .deposit_nation_resource(NATION, material("wood"), 300)
        .unwrap();
    let mut sim = Simulation::new(world).unwrap();
    let subsidy = |settlement_id, amount| NationAction::Subsidize {
        settlement_id,
        gold: 0,
        resources: BTreeMap::from([(material("wood"), amount)]),
    };
    sim.queue_nation_decision(NationDecision::new(
        NATION,
        vec![subsidy(EAST, 250), subsidy(WEST, 100)],
    ))
    .unwrap();
    let report = sim.step().unwrap();
    let actions = &report.nation_decisions[0].results;
    assert!(matches!(
        &actions[0].outcome,
        NationActionOutcome::Applied { effect: NationActionEffect::SubsidyDelivered { delivered, returned, .. } }
        if delivered[&material("wood")] == 100 && returned[&material("wood")] == 150
    ));
    assert!(matches!(
        &actions[1].outcome,
        NationActionOutcome::Applied { effect: NationActionEffect::SubsidyDelivered { delivered, .. } }
        if delivered[&material("wood")] == 100
    ));
    assert_eq!(
        sim.world()
            .nation(NATION)
            .unwrap()
            .stock()
            .amount(material("wood")),
        100
    );
}

fn offer(side: OfferSide, resource: ResourceType, units: u32, price: u32) -> OfferSpec {
    OfferSpec {
        resource,
        side,
        units,
        price_per_unit: price,
    }
}

fn caravan(receiver_id: SettlementId, wood: u32) -> SettlementAction {
    SettlementAction::OrderCaravan {
        receiver_id,
        offers: vec![offer(OfferSide::Sell, material("wood"), wood, 1)],
    }
}

#[test]
fn earlier_caravan_orders_keep_their_reservations_across_decisions() {
    for separate_decisions in [false, true] {
        let mut sim = Simulation::new(setup(false, false)).unwrap();
        let market = SettlementAction::PublishMarketOffers {
            offers: vec![offer(OfferSide::Sell, material("wood"), 4, 1)],
        };
        if separate_decisions {
            queue(&mut sim, vec![caravan(EAST, 7)]);
            queue(&mut sim, vec![caravan(WEST, 7), market]);
        } else {
            queue(&mut sim, vec![caravan(EAST, 7), caravan(WEST, 7), market]);
        }
        let report = sim.step().unwrap();
        let actions: Vec<_> = report.decisions.iter().flat_map(|d| &d.results).collect();
        assert_eq!(
            actions[0].outcome,
            ActionOutcome::Applied {
                effect: ActionEffect::CaravanOrdered { receiver_id: EAST }
            }
        );
        for (action, requested) in actions[1..].iter().zip([7, 4]) {
            assert_eq!(
                action.outcome,
                ActionOutcome::Rejected {
                    error: ActionError::InsufficientStock {
                        resource: material("wood"),
                        requested,
                        available: 3,
                    }
                }
            );
        }
        let report = &report.settlements[&SENDER];
        assert_eq!(report.inventory.trade_reserved[&material("wood")], 7);
        assert_eq!(report.inventory.closing_stock[&material("wood")], 3);
        assert!(report.inventory.is_balanced());
        assert_eq!(sim.world().caravans[0].receiver_id, EAST);
    }
}

#[test]
fn market_priority_and_failed_gold_funding_leave_a_valid_later_order_intact() {
    let mut sim = Simulation::new(setup(false, false)).unwrap();
    let market = SettlementAction::PublishMarketOffers {
        offers: vec![offer(OfferSide::Sell, material("wood"), 3, 1)],
    };
    let unaffordable = SettlementAction::OrderCaravan {
        receiver_id: EAST,
        offers: vec![
            offer(OfferSide::Sell, material("wood"), 7, 1),
            offer(OfferSide::Buy, material("stone"), 3, 7),
        ],
    };
    queue(
        &mut sim,
        vec![market, caravan(WEST, 8), unaffordable, caravan(EAST, 7)],
    );
    let report = sim.step().unwrap();
    let actions = &report.decisions[0].results;
    assert_eq!(
        actions[1].outcome,
        ActionOutcome::Rejected {
            error: ActionError::InsufficientStock {
                resource: material("wood"),
                requested: 8,
                available: 7,
            }
        }
    );
    assert_eq!(
        actions[2].outcome,
        ActionOutcome::Rejected {
            error: ActionError::InsufficientGold {
                requested: 21,
                available: 20,
            }
        }
    );
    assert!(actions[3].is_applied());
    let trade = &report.settlements[&SENDER].trade;
    assert_eq!(trade.market_reserved[&material("wood")], 3);
    assert_eq!(trade.caravans_dispatched.len(), 1);
    assert_eq!(trade.caravans_dispatched[0].cargo[&material("wood")], 7);
    assert_eq!(sim.settlement(SENDER).unwrap().gold(), 20);
    assert!(report.settlements[&SENDER].inventory.is_balanced());
}

#[test]
fn reservations_are_realized_in_fifo_order_and_consumed_once() {
    let mut simulation = Simulation::new(setup(false, false)).unwrap();
    let buy_and_sell = SettlementAction::OrderCaravan {
        receiver_id: WEST,
        offers: vec![
            offer(OfferSide::Sell, material("wood"), 4, 1),
            offer(OfferSide::Buy, material("stone"), 2, 5),
        ],
    };
    queue(&mut simulation, vec![buy_and_sell, caravan(EAST, 3)]);
    let report = simulation.step().unwrap();
    assert!(
        report.decisions[0]
            .results
            .iter()
            .all(|row| row.is_applied())
    );
    let local = &report.settlements[&SENDER];
    assert_eq!(
        local
            .trade
            .caravans_dispatched
            .iter()
            .map(|x| x.receiver_id)
            .collect::<Vec<_>>(),
        [WEST, EAST]
    );
    assert_eq!(local.inventory.trade_reserved[&material("wood")], 7);
    assert!(local.inventory.is_balanced());
    assert_eq!(local.trade.gold_reserved(), 10);
    assert!(
        local
            .trade
            .caravans_dispatched
            .iter()
            .all(|item| item.route == Some(CaravanDepartureRoute::Routed))
    );
    assert_eq!(simulation.settlement(SENDER).unwrap().gold(), 10);
    assert_eq!(
        simulation
            .world()
            .caravans
            .iter()
            .map(|x| x.receiver_id)
            .collect::<Vec<_>>(),
        [WEST, EAST]
    );
    let phase = |phase| {
        report
            .execution
            .iter()
            .position(|event| event.phase == phase && event.settlement_id == SENDER)
            .unwrap()
    };
    assert!(phase(TurnPhase::Decisions) < phase(TurnPhase::EconomicUpdates));
    let next = simulation.step().unwrap();
    assert!(
        next.settlements[&SENDER]
            .trade
            .caravans_dispatched
            .is_empty()
    );
    assert_eq!(next.settlements[&SENDER].trade.gold_reserved(), 0);
    assert!(next.settlements[&SENDER].inventory.is_balanced());
}

#[test]
fn routing_keeps_each_cargo_and_returns_an_unroutable_order_without_another_charge() {
    let mut simulation = Simulation::new(setup(true, false)).unwrap();
    queue(
        &mut simulation,
        vec![
            SettlementAction::OrderCaravan {
                receiver_id: EAST,
                offers: vec![
                    offer(OfferSide::Sell, material("wood"), 7, 1),
                    offer(OfferSide::Buy, material("stone"), 2, 5),
                ],
            },
            caravan(WEST, 2),
        ],
    );
    let first = simulation.step().unwrap();
    assert!(first.decisions[0].results[0].is_applied());
    assert!(first.decisions[0].results[1].is_applied());
    assert_eq!(simulation.world().caravans.len(), 1);
    assert_eq!(simulation.world().caravans[0].receiver_id, WEST);
    let departures = &first.settlements[&SENDER].trade.caravans_dispatched;
    assert_eq!(departures.len(), 2);
    assert_eq!(departures[0].receiver_id, EAST);
    assert_eq!(departures[0].route, Some(CaravanDepartureRoute::NoRoute));
    assert_eq!(departures[0].cargo[&material("wood")], 7);
    assert_eq!(departures[0].gold, 10);
    assert_eq!(departures[1].receiver_id, WEST);
    assert_eq!(departures[1].route, Some(CaravanDepartureRoute::Routed));
    assert_eq!(departures[1].cargo[&material("wood")], 2);
    assert_eq!(departures[1].gold, 0);
    assert!(first.settlements[&SENDER].inventory.is_balanced());
    assert_eq!(
        first.settlements[&SENDER].inventory.trade_reserved[&material("wood")],
        9
    );
    let sender_report = &first.settlements[&SENDER];
    assert_eq!(
        sender_report.trade.reserved(),
        sender_report.inventory.trade_reserved
    );
    assert_eq!(sender_report.trade.gold_reserved(), 10);
    let second = simulation.step().unwrap();
    let report = &second.settlements[&SENDER];
    assert_eq!(
        report.trade.caravans_returned[0].outcome,
        CaravanOutcome::NoRoute
    );
    assert_eq!(report.inventory.closing_stock[&material("wood")], 8);
    assert_eq!(simulation.settlement(SENDER).unwrap().gold(), 20);
    assert!(report.inventory.is_balanced());
}

#[test]
fn rejected_actions_leave_the_target_available_but_applied_actions_claim_it() {
    let mut simulation = Simulation::new(setup(false, false)).unwrap();
    let valid = ResourceCatalog::standard()
        .materials()
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    queue(
        &mut simulation,
        vec![
            SettlementAction::SetStorageOrder { order: vec![] },
            SettlementAction::SetStorageOrder {
                order: valid.clone(),
            },
            SettlementAction::SetStorageOrder {
                order: valid.clone(),
            },
            caravan(EAST, 11),
            caravan(EAST, 10),
        ],
    );
    let report = simulation.step().unwrap();
    let actions = &report.decisions[0].results;
    assert!(matches!(
        actions[0].outcome,
        ActionOutcome::Rejected {
            error: ActionError::InvalidStorageOrder { .. }
        }
    ));
    assert!(actions[1].is_applied());
    assert_eq!(
        actions[2].outcome,
        ActionOutcome::Rejected {
            error: ActionError::Conflict { with_index: 1 }
        }
    );
    assert!(!actions[3].is_applied());
    assert!(actions[4].is_applied());
    assert_eq!(
        simulation.settlement(SENDER).unwrap().storage_order(),
        valid
    );
}
