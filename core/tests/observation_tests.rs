//! Settlement observations: local content, own-province territory and
//! information boundaries between settlements.
use simulation_core::buildings::Building;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::economy::ResourceType::{Grain, Stone, Wood};
use simulation_core::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use simulation_core::settlement::decision::{OfferSide, OfferSpec};
use simulation_core::settlement::observation::{
    BuildingTypeObservation, ProjectTarget, RecipeObservation, SettlementObservation, TileState,
    TradePartner,
};
use simulation_core::settlement::projects::ProjectCost;
use simulation_core::settlement::{NeedRegistry, NeedTier, Settlement};
use simulation_core::simulation::{SettlementAction, SettlementDecision, Simulation};
use simulation_core::trade::offer::TradeOffer;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::elevation::Elevation;
use simulation_core::world::{Map, Position, Province, World, WorldError, terrain::area::Area};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

const ME: SettlementId = SettlementId(1);
const NEIGHBOUR: SettlementId = SettlementId(2);
const MY_FARM: BuildingId = BuildingId(1);
const NEIGHBOUR_FARM: BuildingId = BuildingId(5);

fn at(x: i32, y: i32) -> Position {
    Position { x, y }
}

fn farm() -> BuildingDefinition {
    BuildingDefinition::new(
        "farm",
        "Farm",
        1,
        2,
        vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(Grain, 2)],
        }],
        ProjectCost {
            needed_resources: HashMap::from([(Wood, 2)]),
            turn_cost: 2,
        },
    )
    .unwrap()
}

/// Provinces `10 10 20 20 30 / 10 ~10 20 20 30` (`~` is ocean).
/// Settlement 1 (province 10): population 4 eating 1 Grain each, Wood 10, gold 15,
/// a farm at (0,0), a construction at (1,0) and a Wood market offer.
/// Settlement 2 (province 20): its stock, gold, buildings, projects and offers all
/// depend on `secret`, which settlement 1 must not be able to observe.
fn world(secret: u32) -> World {
    let mut terrain = vec![Area::new(0.5, 0.5); 10];
    terrain[6] = Area::new(0.0, 0.5);
    let mut world = World::new(
        Map::new(5, 2, terrain, vec![10, 10, 20, 20, 30, 10, 10, 20, 20, 30]),
        vec![Province::new(10), Province::new(20), Province::new(30)],
    );
    let mut catalog = BuildingCatalog::default();
    catalog.register(farm());
    world.set_building_catalog(catalog);
    let needs = Arc::new(NeedRegistry::new(vec![NeedTier {
        min_population: 1,
        needs_per_capita: HashMap::from([(Grain, 1)]),
    }]));

    let mut me = Settlement::new(ME, 100, 4, needs);
    me.deposit(Wood, 10);
    me.deposit_gold(15);
    me.publish_trade_offers(HashMap::from([(Wood, TradeOffer::sell(3, 2))]))
        .unwrap();
    world.add_settlement(10, me).unwrap();

    let mut neighbour = Settlement::new(NEIGHBOUR, 100, 3, Arc::default());
    neighbour.deposit(Stone, secret);
    neighbour.deposit_gold(secret * 10);
    if secret > 0 {
        neighbour
            .publish_trade_offers(HashMap::from([(Stone, TradeOffer::sell(secret, 1))]))
            .unwrap();
    }
    world.add_settlement(20, neighbour).unwrap();

    let blueprint = world.building_catalog().get_blueprint("farm").unwrap();
    let mut finder = PathFinder::new(5, 2);
    world
        .add_building(
            ME,
            Building::new(MY_FARM, at(0, 0), blueprint.clone()),
            &mut finder,
        )
        .unwrap();
    world
        .queue_construction(
            ME,
            ProjectId(1),
            at(1, 0),
            blueprint.clone(),
            blueprint.construction_cost().clone(),
        )
        .unwrap();
    if secret > 0 {
        world
            .add_building(
                NEIGHBOUR,
                Building::new(NEIGHBOUR_FARM, at(2, 0), blueprint.clone()),
                &mut finder,
            )
            .unwrap();
        world
            .queue_construction(
                NEIGHBOUR,
                ProjectId(secret),
                at(3, 1),
                blueprint,
                ProjectCost::default(),
            )
            .unwrap();
    }
    world
}

fn observe(simulation: &Simulation) -> SettlementObservation {
    simulation.observe(ME).unwrap()
}

fn decide(simulation: &mut Simulation, id: SettlementId, actions: Vec<SettlementAction>) {
    simulation
        .queue_decision(SettlementDecision::new(id, actions))
        .unwrap();
}

#[test]
fn observation_describes_the_local_economy() {
    let simulation = Simulation::new(world(7)).unwrap();
    let observation = observe(&simulation);
    assert_eq!(observation.turn, 0);
    assert_eq!(observation.settlement_id, ME);
    assert_eq!(observation.province_id, 10);
    assert_eq!(observation.population, 4);
    assert_eq!(observation.gold, 15);
    assert_eq!(observation.inventory.capacity, 100);
    assert_eq!(observation.inventory.stored, BTreeMap::from([(Wood, 10)]));
    assert_eq!(observation.inventory.stored_total, 10);
    assert!(observation.inventory.pending.is_empty());
    assert_eq!(
        observation.inventory.storage_order,
        vec![Wood, Stone, Grain]
    );
    assert_eq!(observation.needs_per_turn, BTreeMap::from([(Grain, 4)]));
    assert_eq!(observation.workforce.available, 4);
    assert_eq!(observation.workforce.requested, 0);

    assert_eq!(observation.buildings.len(), 1);
    let building = &observation.buildings[0];
    assert_eq!(building.building_id, MY_FARM);
    assert_eq!(building.building_type, BuildingTypeId::new("farm"));
    assert_eq!(building.position, at(0, 0));
    assert!(building.active);
    assert_eq!(building.max_workers, 2);
    assert_eq!(building.requested_workers, 0);
    assert_eq!(building.location_productivity, 1.0);

    assert_eq!(observation.projects.len(), 1);
    let project = &observation.projects[0];
    assert_eq!(project.project_id, ProjectId(1));
    assert_eq!(
        project.target,
        ProjectTarget::Construct {
            building_type: BuildingTypeId::new("farm"),
            position: at(1, 0)
        }
    );
    assert_eq!(project.remaining_resources, BTreeMap::from([(Wood, 2)]));
    assert_eq!(project.remaining_turns, 2);
    assert_eq!(observation.next_project_id, Some(ProjectId(2)));

    assert_eq!(
        observation.trade.market_offers,
        vec![OfferSpec {
            resource: Wood,
            side: OfferSide::Sell,
            units: 3,
            price_per_unit: 2
        }]
    );
    assert_eq!(observation.trade.caravans_in_transit, 0);
    assert!(observation.last_turn.is_none());
    assert_eq!(
        observation.trade_partners,
        vec![TradePartner {
            settlement_id: NEIGHBOUR,
            province_id: 20
        }]
    );
    assert_eq!(
        observation.building_types,
        vec![BuildingTypeObservation {
            building_type: BuildingTypeId::new("farm"),
            name: "Farm".into(),
            tier: 1,
            max_workers: 2,
            construction_resources: BTreeMap::from([(Wood, 2)]),
            construction_turns: 2,
            recipes: vec![RecipeObservation {
                inputs: BTreeMap::new(),
                outputs: BTreeMap::from([(Grain, 2)])
            }]
        }]
    );
}

#[test]
fn territory_contains_exactly_the_own_province() {
    let simulation = Simulation::new(world(7)).unwrap();
    let territory = observe(&simulation).territory;
    let tiles: Vec<_> = territory
        .iter()
        .map(|tile| (tile.position, tile.occupancy, tile.buildable))
        .collect();
    assert_eq!(
        tiles,
        vec![
            (
                at(0, 0),
                TileState::OwnBuilding {
                    building_id: MY_FARM
                },
                false
            ),
            (
                at(1, 0),
                TileState::OwnConstruction {
                    project_id: ProjectId(1)
                },
                false
            ),
            (at(0, 1), TileState::Empty, true),
            (at(1, 1), TileState::Empty, false),
        ]
    );
    assert_eq!(territory[3].elevation, Elevation::Ocean);
    assert_eq!(territory[2].elevation, Elevation::Plains);
}

#[test]
fn other_settlements_internals_do_not_change_the_observation() {
    let quiet = Simulation::new(world(0)).unwrap();
    let busy = Simulation::new(world(50)).unwrap();
    assert_eq!(observe(&quiet), observe(&busy));
    // The neighbour's own observation does differ, so the setup is effective.
    assert_ne!(
        quiet.observe(NEIGHBOUR).unwrap(),
        busy.observe(NEIGHBOUR).unwrap()
    );
}

#[test]
fn other_settlements_decisions_stay_hidden_across_turns() {
    let mut quiet = Simulation::new(world(0)).unwrap();
    let mut busy = Simulation::new(world(50)).unwrap();
    for turn in 0..3 {
        for simulation in [&mut quiet, &mut busy] {
            decide(
                simulation,
                ME,
                vec![SettlementAction::SetBuildingWorkers {
                    building_id: MY_FARM,
                    workers: turn % 3,
                }],
            );
        }
        decide(
            &mut busy,
            NEIGHBOUR,
            vec![
                SettlementAction::SetBuildingWorkers {
                    building_id: NEIGHBOUR_FARM,
                    workers: 2,
                },
                SettlementAction::Construct {
                    building_type: BuildingTypeId::new("farm"),
                    position: at(2, 1),
                },
                SettlementAction::ClearMarketOffers,
            ],
        );
        quiet.step().unwrap();
        busy.step().unwrap();
        assert_eq!(observe(&quiet), observe(&busy), "turn {turn}");
    }
    // The neighbour's decisions did take effect.
    let neighbour = busy.observe(NEIGHBOUR).unwrap();
    let farm = neighbour
        .buildings
        .iter()
        .find(|building| building.building_id == NEIGHBOUR_FARM)
        .unwrap();
    assert_eq!(farm.requested_workers, 2);
    assert!(neighbour.trade.market_offers.is_empty());
    assert!(
        neighbour
            .projects
            .iter()
            .any(|project| matches!(project.target, ProjectTarget::Construct { position, .. } if position == at(2, 1)))
    );
}

#[test]
fn serialized_observation_mentions_no_other_settlement_beyond_its_id() {
    let simulation = Simulation::new(world(50)).unwrap();
    let json = observe(&simulation).to_json();
    // The neighbour appears once, as a trade partner.
    assert_eq!(json.matches(r#""settlement_id":2"#).count(), 1);
    // No tile, building or project outside province 10 (x = 0 or 1).
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mut xs = Vec::new();
    collect_x(&value, &mut xs);
    assert!(!xs.is_empty());
    assert!(xs.iter().all(|x| *x <= 1), "{xs:?}");
    // The neighbour's stone, gold and offers are nowhere in the observation.
    assert!(!json.contains("Stone\":50"));
    assert!(!json.contains("500"));
}

fn collect_x(value: &serde_json::Value, xs: &mut Vec<i64>) {
    match value {
        serde_json::Value::Object(map) => {
            if let (Some(x), true) = (map.get("x"), map.contains_key("y")) {
                xs.push(x.as_i64().unwrap());
            }
            map.values().for_each(|value| collect_x(value, xs));
        }
        serde_json::Value::Array(items) => items.iter().for_each(|value| collect_x(value, xs)),
        _ => {}
    }
}

#[test]
fn a_foreign_structure_in_the_own_province_is_anonymous() {
    let mut world = world(50);
    // A border change moves the neighbour's farm into province 10.
    world.set_tile_province(at(2, 0), 10).unwrap();
    let simulation = Simulation::new(world).unwrap();
    let observation = observe(&simulation);
    let tile = observation
        .territory
        .iter()
        .find(|tile| tile.position == at(2, 0))
        .unwrap();
    assert_eq!(tile.occupancy, TileState::Foreign);
    assert!(!tile.buildable);
    assert!(
        observation
            .buildings
            .iter()
            .all(|building| building.building_id != NEIGHBOUR_FARM)
    );
}

#[test]
fn last_turn_summary_follows_each_step() {
    let mut simulation = Simulation::new(world(0)).unwrap();
    decide(
        &mut simulation,
        ME,
        vec![SettlementAction::SetBuildingWorkers {
            building_id: MY_FARM,
            workers: 2,
        }],
    );
    simulation.step().unwrap();
    let observation = observe(&simulation);
    assert_eq!(observation.turn, 1);
    assert_eq!(observation.workforce.requested, 2);
    assert_eq!(observation.workforce.employed, 2);
    assert_eq!(observation.buildings[0].current_workers, 2);
    let last = observation.last_turn.unwrap();
    // Grain is produced after consumption, so this turn's needs were not met.
    assert_eq!(last.missing_needs, vec![Grain]);
    assert_eq!(last.satisfaction_percent, 0.0);
    assert_eq!(last.produced, BTreeMap::from([(Grain, 2)]));
    assert_eq!(last.project_consumed, BTreeMap::from([(Wood, 2)]));
    assert_eq!(last.trade_reserved, BTreeMap::from([(Wood, 3)]));
    assert_eq!(last.trade_received, BTreeMap::from([(Wood, 3)]));
    assert!(last.completed_projects.is_empty());
    assert_eq!(
        observation.inventory.stored,
        BTreeMap::from([(Grain, 2), (Wood, 8)])
    );

    simulation.step().unwrap();
    let observation = observe(&simulation);
    assert_eq!(observation.turn, 2);
    let last = observation.last_turn.unwrap();
    assert_eq!(last.completed_projects, vec![ProjectId(1)]);
    assert!(observation.projects.is_empty());
    assert_eq!(observation.buildings.len(), 2);
    assert!(matches!(
        observation.territory[1].occupancy,
        TileState::OwnBuilding { .. }
    ));
}

#[test]
fn own_caravans_are_counted_but_not_their_trading_partners_books() {
    let mut simulation = Simulation::new(world(0)).unwrap();
    decide(
        &mut simulation,
        ME,
        vec![SettlementAction::OrderCaravan {
            receiver_id: NEIGHBOUR,
            offers: vec![OfferSpec {
                resource: Wood,
                side: OfferSide::Sell,
                units: 2,
                price_per_unit: 1,
            }],
        }],
    );
    simulation.step().unwrap();
    let observation = observe(&simulation);
    assert_eq!(observation.trade.caravans_in_transit, 1);
    assert_eq!(
        simulation
            .observe(NEIGHBOUR)
            .unwrap()
            .trade
            .caravans_in_transit,
        0
    );
}

#[test]
fn observations_round_trip_through_json() {
    let mut simulation = Simulation::new(world(50)).unwrap();
    simulation.step().unwrap();
    for observation in simulation.observe_all().into_values() {
        let back: SettlementObservation = serde_json::from_str(&observation.to_json()).unwrap();
        assert_eq!(back, observation);
    }
}

#[test]
fn every_registered_settlement_can_be_observed_and_others_cannot() {
    let simulation = Simulation::new(world(1)).unwrap();
    let all = simulation.observe_all();
    assert_eq!(all.keys().copied().collect::<Vec<_>>(), vec![ME, NEIGHBOUR]);
    assert_eq!(all[&ME], observe(&simulation));
    assert_eq!(
        simulation.observe(SettlementId(9)),
        Err(WorldError::MissingSettlement(SettlementId(9)))
    );
}
