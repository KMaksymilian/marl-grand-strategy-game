use super::trade_network::TradeNetwork;
use simulation_core::buildings::definition::BuildingDefinition;
use simulation_core::economy::ResourceType::{Grain, Stone, Wood};
use simulation_core::ids::ProjectId;
use simulation_core::ids::SettlementId;
use simulation_core::settlement::{Settlement, projects::ProjectCost};
use simulation_core::trade::offer::TradeOffer;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::area::Area;
use simulation_core::world::territory::TileOccupancy;
use simulation_core::world::{Map, Point, Position, Province, World, WorldError};
use std::collections::HashMap;
use std::sync::Arc;

fn definition() -> Arc<BuildingDefinition> {
    Arc::new(
        BuildingDefinition::new("house", "House", 1, 0, vec![], ProjectCost::default()).unwrap(),
    )
}

fn scenario(capacity: u32) -> (World, HashMap<SettlementId, Point>, PathFinder) {
    let mut world = World::new(
        Map::new(4, 1, vec![Area::new(0.5, 0.5); 4], vec![10, 10, 20, 20]),
        vec![Province::new(10), Province::new(20)],
    );
    let mut buyer = Settlement::new(SettlementId(1), capacity, 0, Arc::default());
    buyer.deposit_gold(20);
    buyer.deposit(Wood, 3);
    buyer.set_storage_order([Stone, Wood, Grain]).unwrap();
    buyer
        .order_caravan(
            SettlementId(2),
            HashMap::from([(Stone, TradeOffer::buy(4, 2))]),
        )
        .unwrap();
    let mut seller = Settlement::new(SettlementId(2), 100, 0, Arc::default());
    seller.deposit(Stone, 4);
    seller
        .publish_trade_offers(HashMap::from([(Stone, TradeOffer::sell(4, 2))]))
        .unwrap();
    // Deliberately reverse insertion order and use non-default capitals.
    world
        .add_settlement_at(20, Position { x: 3, y: 0 }, seller)
        .unwrap();
    world
        .add_settlement_at(10, Position { x: 1, y: 0 }, buyer)
        .unwrap();
    (
        world,
        HashMap::from([
            (SettlementId(1), Point(1, 0)),
            (SettlementId(2), Point(3, 0)),
        ]),
        PathFinder::new(4, 1),
    )
}

#[test]
fn global_coordinator_matches_the_network_for_trade_and_construction() {
    let (mut local, locations, mut finder) = scenario(100);
    let (mut global, _, _) = scenario(100);
    for world in [&mut local, &mut global] {
        world
            .queue_construction(
                SettlementId(1),
                ProjectId(7),
                Position { x: 1, y: 0 },
                definition(),
                ProjectCost {
                    needed_resources: HashMap::from([(Wood, 2)]),
                    turn_cost: 2,
                },
            )
            .unwrap();
    }
    let mut simulation = simulation_core::simulation::Simulation::new(global).unwrap();
    for turn in 1..=4 {
        let local_reports =
            TradeNetwork::process_turn(&mut local, &locations, &mut finder).unwrap();
        let global_report = simulation.step().unwrap();
        assert_eq!(global_report.turn, turn);
        if turn == 1 {
            assert_eq!(simulation.world().caravans[0].position(), Point(1, 0));
        }
        if turn == 2 {
            assert_eq!(simulation.world().caravans[0].position(), Point(3, 0));
        }
        assert_eq!(
            global_report.settlements.values().collect::<Vec<_>>(),
            local_reports.iter().collect::<Vec<_>>()
        );
        assert!(
            global_report
                .settlements
                .values()
                .all(|report| report.inventory.is_balanced())
        );
    }
    assert_eq!(
        simulation
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Stone),
        4
    );
    assert_eq!(
        simulation
            .settlement(SettlementId(1))
            .unwrap()
            .buildings()
            .count(),
        1
    );
}

#[test]
fn construction_and_caravan_trade_use_the_same_settlement_and_balance_stock() {
    let (mut world, locations, mut finder) = scenario(100);
    let site = Position { x: 1, y: 0 };
    world
        .queue_construction(
            SettlementId(1),
            ProjectId(7),
            site,
            definition(),
            ProjectCost {
                needed_resources: HashMap::from([(Wood, 2)]),
                turn_cost: 1,
            },
        )
        .unwrap();
    let first = TradeNetwork::process_turn(&mut world, &locations, &mut finder).unwrap();
    assert_eq!(first[0].inventory.project_consumed[&Wood], 2);
    assert_eq!(first[0].trade.caravans_dispatched.len(), 1);
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .buildings()
            .count(),
        1
    );
    assert!(matches!(
        world.tile_at(site).unwrap().occupancy(),
        TileOccupancy::Building(_)
    ));
    for _ in 0..3 {
        let reports = TradeNetwork::process_turn(&mut world, &locations, &mut finder).unwrap();
        assert!(reports.iter().all(|report| report.inventory.is_balanced()));
    }
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Stone),
        4
    );
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Wood),
        1
    );
    assert_eq!(world.settlement(SettlementId(1)).unwrap().gold(), 12);
    assert_eq!(world.settlement(SettlementId(2)).unwrap().gold(), 8);
}

#[test]
fn canceling_a_project_preserves_trade_and_return_overflow_is_reported() {
    let (mut world, locations, mut finder) = scenario(3);
    let site = Position { x: 1, y: 0 };
    world
        .queue_construction(
            SettlementId(1),
            ProjectId(7),
            site,
            definition(),
            ProjectCost {
                needed_resources: HashMap::from([(Wood, 2)]),
                turn_cost: 2,
            },
        )
        .unwrap();
    world.cancel_project(SettlementId(1), ProjectId(7)).unwrap();
    let mut lost = 0;
    for _ in 0..4 {
        let reports = TradeNetwork::process_turn(&mut world, &locations, &mut finder).unwrap();
        assert!(reports.iter().all(|report| report.inventory.is_balanced()));
        lost += reports[0]
            .inventory
            .losses
            .iter()
            .map(|loss| loss.amount)
            .sum::<u64>();
    }
    assert_eq!(
        world.tile_at(site).unwrap().occupancy(),
        TileOccupancy::Empty
    );
    assert!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .active_projects()
            .is_empty()
    );
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Wood),
        3
    );
    assert_eq!(lost, 4);
    assert_eq!(world.settlement(SettlementId(2)).unwrap().gold(), 8);
}

#[test]
fn invalid_membership_blocks_the_whole_turn_before_funding_or_movement() {
    let (mut world, locations, mut finder) = scenario(100);
    let site = Position { x: 0, y: 0 };
    world
        .queue_construction(
            SettlementId(1),
            ProjectId(7),
            site,
            definition(),
            ProjectCost {
                needed_resources: HashMap::from([(Wood, 2)]),
                turn_cost: 1,
            },
        )
        .unwrap();
    world.set_tile_province(site, 20).unwrap();
    assert!(matches!(
        TradeNetwork::process_turn(&mut world, &locations, &mut finder),
        Err(WorldError::SettlementOutsideProvince { .. })
    ));
    assert!(world.caravans.is_empty());
    assert_eq!(world.settlement(SettlementId(1)).unwrap().gold(), 20);
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Wood),
        3
    );
    assert_eq!(
        world.settlement(SettlementId(1)).unwrap().active_projects()[0]
            .remaining_cost()
            .turn_cost,
        1
    );
}
