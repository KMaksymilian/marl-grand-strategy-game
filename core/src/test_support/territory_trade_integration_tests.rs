use super::trade_network::TradeNetwork;
use crate::test_support::material;
use simulation_core::buildings::definition::BuildingDefinition;
use simulation_core::domain::intelligence::{Information, MarketOffer, MarketOfferSide};
use simulation_core::economy::ResourceCatalog;
use simulation_core::ids::ProjectId;
use simulation_core::ids::SettlementId;
use simulation_core::settlement::observation::SettlementObservation;
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
    buyer.deposit(material("wood"), 3);
    let mut priority = ResourceCatalog::standard().materials().to_vec();
    priority.swap(0, 1);
    buyer.set_storage_order(priority).unwrap();
    buyer
        .order_caravan(
            SettlementId(2),
            HashMap::from([(material("stone"), TradeOffer::buy(4, 2))]),
        )
        .unwrap();
    let mut seller = Settlement::new(SettlementId(2), 100, 0, Arc::default());
    seller.deposit(material("stone"), 4);
    seller
        .publish_trade_offers(HashMap::from([(material("stone"), TradeOffer::sell(4, 2))]))
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
                    needed_resources: HashMap::from([(material("wood"), 2)]),
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
        // The network has no nations; the global turn adds a taxation report
        // (nothing here is produced, so no materials are taxed).
        let global_economy: Vec<_> = global_report
            .settlements
            .values()
            .map(|report| {
                assert!(report.inventory.taxed.is_empty());
                let mut report = report.clone();
                report.taxation = None;
                report
            })
            .collect();
        assert_eq!(
            global_economy.iter().collect::<Vec<_>>(),
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
            .amount(material("stone")),
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
fn visiting_caravan_records_prefunded_market_offers_even_without_a_trade() {
    for price in [2, 3] {
        let (mut world, _, _) = scenario(100);
        let market = SettlementId(2);
        world
            .settlement_mut(market)
            .unwrap()
            .publish_trade_offers(HashMap::from([(
                crate::test_support::material("stone"),
                TradeOffer::sell(6, price),
            )]))
            .unwrap();
        let mut simulation = simulation_core::simulation::Simulation::new(world).unwrap();

        // Caravans also carry troop observations of their route (MGS-45);
        // only the market snapshots matter here.
        let markets = |caravan: &crate::trade::caravan::Caravan| {
            caravan
                .observations()
                .iter()
                .filter(|snapshot| {
                    matches!(snapshot.information(), Information::ForeignMarket { .. })
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        simulation.step().unwrap();
        assert!(markets(&simulation.world().caravans[0]).is_empty());

        let report = simulation.step().unwrap();
        let caravan = &simulation.world().caravans[0];
        let [snapshot] = markets(caravan).try_into().unwrap_or_else(|_| {
            panic!("expected one observation from the market visit");
        });
        assert_eq!(snapshot.observed_turn(), report.turn);
        assert_eq!(
            snapshot.information(),
            &Information::ForeignMarket {
                market,
                province_id: 20,
                offers: [(
                    crate::test_support::material("stone"),
                    MarketOffer {
                        side: MarketOfferSide::Sell,
                        units: 4,
                        unit_price_limit: price,
                    },
                )]
                .into(),
            }
        );
        assert_eq!(caravan.trades().is_empty(), price == 3);
        let sender = SettlementId(1);
        assert!(
            simulation
                .observe(sender)
                .unwrap()
                .trade
                .known_markets
                .is_empty()
        );

        let mut simulation =
            simulation_core::simulation::Simulation::new(simulation.into_world()).unwrap();
        assert_eq!(simulation.current_turn(), report.turn);
        assert_eq!(simulation.step().unwrap().turn, report.turn + 1);
        let observation = simulation.observe(sender).unwrap();
        let known = &observation.trade.known_markets;
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].market, market);
        assert_eq!(known[0].province_id, 20);
        assert_eq!(known[0].observed_turn, report.turn);
        assert_eq!(
            known[0].offers[&crate::test_support::material("stone")].units,
            4
        );
        assert_eq!(
            known[0].offers[&crate::test_support::material("stone")].unit_price_limit,
            price
        );
        assert_eq!(
            known[0].offers[&crate::test_support::material("stone")].side,
            simulation_core::application::commands::settlement::contract::OfferSide::Sell
        );
        assert!(
            simulation
                .observe(market)
                .unwrap()
                .trade
                .known_markets
                .is_empty()
        );
        assert_eq!(
            serde_json::from_str::<SettlementObservation>(&observation.to_json(),).unwrap(),
            observation
        );
    }
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
                needed_resources: HashMap::from([(material("wood"), 2)]),
                turn_cost: 1,
            },
        )
        .unwrap();
    let first = TradeNetwork::process_turn(&mut world, &locations, &mut finder).unwrap();
    assert_eq!(first[0].inventory.project_consumed[&material("wood")], 2);
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
            .amount(material("stone")),
        4
    );
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(material("wood")),
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
                needed_resources: HashMap::from([(material("wood"), 2)]),
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
            .amount(material("wood")),
        3
    );
    assert_eq!(lost, 4);
    assert_eq!(world.settlement(SettlementId(2)).unwrap().gold(), 8);
}

#[test]
fn invalid_membership_blocks_the_turn_without_consuming_accepted_escrow() {
    let (mut world, locations, mut finder) = scenario(100);
    let site = Position { x: 0, y: 0 };
    world
        .queue_construction(
            SettlementId(1),
            ProjectId(7),
            site,
            definition(),
            ProjectCost {
                needed_resources: HashMap::from([(material("wood"), 2)]),
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
    let sender = world.settlement(SettlementId(1)).unwrap();
    assert_eq!(sender.gold(), 12);
    assert_eq!(sender.caravan_orders().len(), 1);
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(material("wood")),
        3
    );
    assert_eq!(
        world.settlement(SettlementId(1)).unwrap().active_projects()[0]
            .remaining_cost()
            .turn_cost,
        1
    );
}
