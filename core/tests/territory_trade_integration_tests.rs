use simulation_core::buildings::definition::BuildingDefinition;
use simulation_core::economy::ResourceType::{Grain, Stone, Wood};
use simulation_core::ids::ProjectId;
use simulation_core::settlement::{Settlement, projects::ProjectCost};
use simulation_core::trade::{network::TradeNetwork, offer::TradeOffer};
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::area::Area;
use simulation_core::world::territory::TileOccupancy;
use simulation_core::world::{Map, Point, Position, Province, World, WorldError};
use std::collections::HashMap;
use std::sync::Arc;

fn definition() -> Arc<BuildingDefinition> {
    Arc::new(BuildingDefinition {
        id: "house".into(),
        name: "House".into(),
        tier: 1,
        max_workers: 0,
        behaviors: vec![],
        construction_cost: ProjectCost::default(),
    })
}

fn scenario(capacity: u32) -> (World, HashMap<u32, Point>, PathFinder) {
    let mut world = World::new(
        Map::new(4, 1, vec![Area::new(0.5, 0.5); 4], vec![10, 10, 20, 20]),
        vec![Province::new(10), Province::new(20)],
    );
    let mut buyer = Settlement::new(1, capacity, 0, Arc::default());
    buyer.deposit_gold(20);
    buyer.deposit(Wood, 3);
    buyer.set_storage_order([Stone, Wood, Grain]).unwrap();
    buyer
        .order_caravan(2, HashMap::from([(Stone, TradeOffer::buy(4, 2))]))
        .unwrap();
    let mut seller = Settlement::new(2, 100, 0, Arc::default());
    seller.deposit(Stone, 4);
    seller
        .publish_trade_offers(HashMap::from([(Stone, TradeOffer::sell(4, 2))]))
        .unwrap();
    world.add_settlement(20, seller).unwrap(); // deliberately reverse insertion order
    world.add_settlement(10, buyer).unwrap();
    (
        world,
        HashMap::from([(1, Point(0, 0)), (2, Point(3, 0))]),
        PathFinder::new(4, 1),
    )
}

#[test]
fn construction_and_caravan_trade_use_the_same_settlement_and_balance_stock() {
    let (mut world, locations, mut finder) = scenario(100);
    let site = Position { x: 1, y: 0 };
    world
        .queue_construction(
            1,
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
    assert_eq!(world.settlement(1).unwrap().buildings().count(), 1);
    assert!(matches!(
        world.tile_at(site).unwrap().occupancy(),
        TileOccupancy::Building(_)
    ));
    for _ in 0..3 {
        let reports = TradeNetwork::process_turn(&mut world, &locations, &mut finder).unwrap();
        assert!(reports.iter().all(|report| report.inventory.is_balanced()));
    }
    assert_eq!(world.settlement(1).unwrap().inventory().amount(Stone), 4);
    assert_eq!(world.settlement(1).unwrap().inventory().amount(Wood), 1);
    assert_eq!(world.settlement(1).unwrap().gold(), 12);
    assert_eq!(world.settlement(2).unwrap().gold(), 8);
}

#[test]
fn canceling_a_project_preserves_trade_and_return_overflow_is_reported() {
    let (mut world, locations, mut finder) = scenario(3);
    let site = Position { x: 1, y: 0 };
    world
        .queue_construction(
            1,
            ProjectId(7),
            site,
            definition(),
            ProjectCost {
                needed_resources: HashMap::from([(Wood, 2)]),
                turn_cost: 2,
            },
        )
        .unwrap();
    world.cancel_project(1, ProjectId(7)).unwrap();
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
    assert!(world.settlement(1).unwrap().active_projects().is_empty());
    assert_eq!(world.settlement(1).unwrap().inventory().amount(Wood), 3);
    assert_eq!(lost, 4);
    assert_eq!(world.settlement(2).unwrap().gold(), 8);
}

#[test]
fn invalid_membership_blocks_the_whole_turn_before_funding_or_movement() {
    let (mut world, locations, mut finder) = scenario(100);
    let site = Position { x: 1, y: 0 };
    world
        .queue_construction(
            1,
            ProjectId(7),
            site,
            definition(),
            ProjectCost {
                needed_resources: HashMap::from([(Wood, 2)]),
                turn_cost: 1,
            },
        )
        .unwrap();
    world.map.territory[1] = 20;
    assert!(matches!(
        TradeNetwork::process_turn(&mut world, &locations, &mut finder),
        Err(WorldError::SettlementOutsideProvince { .. })
    ));
    assert!(world.caravans.is_empty());
    assert_eq!(world.settlement(1).unwrap().gold(), 20);
    assert_eq!(world.settlement(1).unwrap().inventory().amount(Wood), 3);
    assert_eq!(
        world.settlement(1).unwrap().active_projects()[0]
            .remaining_cost()
            .turn_cost,
        1
    );
}
