mod common;

use common::material;
use simulation_core::buildings::Building;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::buildings::passive::{BuildingRuleError, TradeLimitBonuses};
use simulation_core::configuration::Configuration;
use simulation_core::economy::ResourceCatalog;
use simulation_core::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use simulation_core::settlement::Settlement;
use simulation_core::settlement::decision::{
    ActionError, ActionOutcome, OfferSide, OfferSpec, SettlementAction, SettlementDecision,
};
use simulation_core::settlement::projects::{ProjectCost, ProjectError};
use simulation_core::simulation::Simulation;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::area::Area;
use simulation_core::world::territory::TileOccupancy;
use simulation_core::world::{Map, Position, Province, World, WorldError};
use std::sync::Arc;

const OWNER: SettlementId = SettlementId(1);

fn position(x: i32) -> Position {
    Position { x, y: 0 }
}

fn world() -> World {
    let mut world = World::new(
        Map::new(8, 1, vec![Area::new(0.5, 0.5); 8], vec![1; 8]),
        vec![Province::new(1)],
    );
    world
        .add_settlement_at(
            1,
            position(0),
            Settlement::new(OWNER, 100, 1, Arc::default()),
        )
        .unwrap();
    world
}

fn catalog() -> BuildingCatalog {
    BuildingCatalog::from_toml(
        include_str!("../../buildings.toml"),
        ResourceCatalog::standard(),
    )
    .unwrap()
}

fn advance(world: World) -> World {
    let mut simulation = Simulation::new(world).unwrap();
    simulation.step().unwrap();
    simulation.into_world()
}

fn build(
    world: &mut World,
    project_id: u32,
    x: i32,
    definition: &Arc<BuildingDefinition>,
) -> Result<(), WorldError> {
    world.queue_construction(
        OWNER,
        ProjectId(project_id),
        position(x),
        definition.clone(),
        ProjectCost::default(),
    )
}

#[test]
fn passive_warehouse_and_market_stack_and_demolition_preserves_stock() {
    let mut world = world();
    let catalog = catalog();
    let warehouse = catalog.get_blueprint("warehouse").unwrap();
    let market = catalog.get_blueprint("market").unwrap();
    let mut finder = PathFinder::new(8, 1);
    for (id, definition) in [(0, &warehouse), (1, &warehouse), (2, &market), (3, &market)] {
        world
            .add_building(
                OWNER,
                Building::new(BuildingId(id), position(id as i32), definition.clone()),
                &mut finder,
            )
            .unwrap();
    }
    let producer = Arc::new(
        BuildingDefinition::new(
            "producer",
            "Producer",
            1,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(material("wood"), 300)],
            }],
            ProjectCost::default(),
        )
        .unwrap(),
    );
    let mut building = Building::new(BuildingId(4), position(4), producer);
    building.set_target_load(1.0).unwrap();
    world.add_building(OWNER, building, &mut finder).unwrap();

    let mut world = advance(world);
    let settlement = world.settlement(OWNER).unwrap();
    assert_eq!(settlement.inventory().capacity(), 400_100);
    assert_eq!(settlement.pending_inventory().capacity(), 400_100);
    assert_eq!(
        settlement.trade_limit_bonuses(),
        TradeLimitBonuses {
            caravan_slots: 4,
            offer_slots: 8,
            caravan_cargo: 80_000,
            open_offer_units: 160_000,
        }
    );
    let stored = settlement.inventory().amount(material("wood"));
    assert!(stored > 100);

    for id in [0, 1, 2] {
        world.demolish_building(OWNER, BuildingId(id)).unwrap();
        assert_eq!(
            world.tile_at(position(id as i32)).unwrap().occupancy(),
            TileOccupancy::Empty
        );
    }

    let settlement = world.settlement(OWNER).unwrap();
    assert_eq!(settlement.inventory().capacity(), 100);
    assert_eq!(settlement.pending_inventory().capacity(), 100);
    assert_eq!(settlement.inventory().amount(material("wood")), stored);
    assert_eq!(
        settlement.trade_limit_bonuses(),
        market.passive_rules().effects.trade
    );

    world.demolish_building(OWNER, BuildingId(4)).unwrap();
    let workshop = Arc::new(
        BuildingDefinition::new(
            "test_workshop",
            "Test Workshop",
            1,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![(material("wood"), 80), (material("stone"), 80)],
                outputs: vec![(material("planks"), 80)],
            }],
            ProjectCost::default(),
        )
        .unwrap(),
    );
    let mut building = Building::new(BuildingId(4), position(4), workshop);
    building.set_target_load(1.0).unwrap();
    world.add_building(OWNER, building, &mut finder).unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    simulation
        .queue_decision(SettlementDecision::new(
            OWNER,
            vec![SettlementAction::PublishMarketOffers {
                offers: vec![OfferSpec {
                    resource: material("wood"),
                    side: OfferSide::Sell,
                    units: 70,
                    price_per_unit: 1,
                }],
            }],
        ))
        .unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.settlements[&OWNER].inventory.returned[&material("wood")],
        80
    );
    assert_eq!(
        report.settlements[&OWNER].trade.market_unsold[&material("wood")],
        70
    );
    assert!(report.settlements[&OWNER].inventory.losses.is_empty());
    assert_eq!(
        simulation.observe(OWNER).unwrap().inventory.stored[&material("wood")],
        stored
    );
}

#[test]
fn monument_limit_reserves_active_projects_and_demolition_frees_a_slot() {
    let mut world = world();
    let monument = catalog().get_blueprint("monument").unwrap();
    for index in 0..4 {
        build(&mut world, index as u32, index, &monument).unwrap();
    }
    assert_eq!(
        world.settlement(OWNER).unwrap().monument_happiness_bonus(),
        0
    );
    assert_eq!(
        build(&mut world, 4, 4, &monument),
        Err(WorldError::Project(ProjectError::BuildingRule(
            BuildingRuleError::LimitReached {
                group: "monuments".to_owned(),
                maximum: 4,
            }
        )))
    );
    let mut world = advance(world);
    assert_eq!(
        world.settlement(OWNER).unwrap().monument_happiness_bonus(),
        20
    );

    world.demolish_building(OWNER, BuildingId(0)).unwrap();
    assert_eq!(
        world.settlement(OWNER).unwrap().monument_happiness_bonus(),
        15
    );
    build(&mut world, 4, 4, &monument).unwrap();
    let world = advance(world);
    assert_eq!(
        world.settlement(OWNER).unwrap().monument_happiness_bonus(),
        20
    );
}

#[test]
fn walls_upgrade_one_level_at_a_time_and_replace_the_defense_bonus() {
    let mut world = world();
    let catalog = catalog();
    let level_one = catalog.get_blueprint("wall_1").unwrap();
    let level_two = catalog.get_blueprint("wall_2").unwrap();
    let level_three = catalog.get_blueprint("wall_3").unwrap();
    for (wall, turns, cost) in [
        (&level_one, 3, 20_000),
        (&level_two, 5, 40_000),
        (&level_three, 8, 80_000),
    ] {
        assert_eq!(wall.construction_cost().turn_cost, turns);
        assert_eq!(
            wall.construction_cost().needed_resources[&material("fortification_components")],
            cost
        );
    }
    let upgrade = |world: &mut World, project, definition: &Arc<BuildingDefinition>| {
        world.queue_upgrade(
            OWNER,
            ProjectId(project),
            BuildingId(0),
            definition.clone(),
            ProjectCost::default(),
        )
    };
    let requires = |id: &str| {
        Err(WorldError::Project(ProjectError::BuildingRule(
            BuildingRuleError::UpgradeRequires {
                previous_type: id.to_owned(),
            },
        )))
    };

    assert_eq!(build(&mut world, 0, 0, &level_two), requires("wall_1"));
    build(&mut world, 0, 0, &level_one).unwrap();
    let mut world = advance(world);
    assert_eq!(
        world
            .province_fortification_defense_basis_points(1)
            .unwrap(),
        2500
    );
    assert_eq!(upgrade(&mut world, 1, &level_three), requires("wall_2"));
    upgrade(&mut world, 1, &level_two).unwrap();
    assert_eq!(
        upgrade(&mut world, 2, &level_two),
        Err(WorldError::Project(ProjectError::BuildingAlreadyUpgrading(
            BuildingId(0)
        )))
    );
    let mut world = advance(world);
    assert_eq!(
        world
            .province_fortification_defense_basis_points(1)
            .unwrap(),
        5000
    );
    upgrade(&mut world, 2, &level_three).unwrap();
    let mut world = advance(world);
    assert_eq!(
        world
            .province_fortification_defense_basis_points(1)
            .unwrap(),
        10000
    );

    world.demolish_building(OWNER, BuildingId(0)).unwrap();
    assert_eq!(
        world
            .province_fortification_defense_basis_points(1)
            .unwrap(),
        0
    );
}

#[test]
fn configured_market_capacity_is_enforced_and_demolition_trims_funded_offers() {
    let mut world = World::new(
        Map::new(
            8,
            1,
            vec![Area::new(0.5, 0.5); 8],
            vec![1, 1, 1, 1, 1, 1, 1, 2],
        ),
        vec![Province::new(1), Province::new(2)],
    );
    let configuration = Configuration::from_toml(include_str!("../../configuration.toml")).unwrap();
    world.set_base_trade_limits(configuration.economy.trade_limits);
    world.set_building_catalog(catalog());
    let mut settlement = Settlement::new(OWNER, 300_000, 1, Arc::default());
    settlement.deposit(material("wood"), 120_000);
    settlement.deposit(material("stone"), 100_000);
    world.add_settlement_at(1, position(0), settlement).unwrap();
    let trading_partner = SettlementId(2);
    world
        .add_settlement_at(
            2,
            position(7),
            Settlement::new(trading_partner, 100, 1, Arc::default()),
        )
        .unwrap();
    let market = world.building_catalog().get_blueprint("market").unwrap();
    world
        .add_building(
            OWNER,
            Building::new(BuildingId(0), position(1), market),
            &mut PathFinder::new(8, 1),
        )
        .unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    let sell = |resource, units| OfferSpec {
        resource: material(resource),
        side: OfferSide::Sell,
        units,
        price_per_unit: 1,
    };
    let offers = || SettlementAction::PublishMarketOffers {
        offers: vec![sell("wood", 90_000)],
    };
    let caravan = |units| SettlementAction::OrderCaravan {
        receiver_id: trading_partner,
        offers: vec![sell("stone", units)],
    };
    simulation
        .queue_decision(SettlementDecision::new(
            OWNER,
            vec![offers(), caravan(50_000), caravan(30_001)],
        ))
        .unwrap();
    let report = simulation.step().unwrap();
    let results = &report.decisions[0].results;
    assert!(results[0].is_applied());
    assert!(results[1].is_applied());
    assert_eq!(
        results[2].outcome,
        ActionOutcome::Rejected {
            error: ActionError::CaravanCapacityExceeded
        }
    );
    assert_eq!(
        report.settlements[&OWNER].trade.market_reserved[&material("wood")],
        90_000
    );

    simulation
        .queue_decision(SettlementDecision::new(
            OWNER,
            vec![
                SettlementAction::Demolish {
                    building_id: BuildingId(0),
                },
                offers(),
            ],
        ))
        .unwrap();
    let report = simulation.step().unwrap();
    assert!(report.decisions[0].results[0].is_applied());
    assert_eq!(
        report.decisions[0].results[1].outcome,
        ActionOutcome::Rejected {
            error: ActionError::MarketCapacityExceeded
        }
    );
    assert_eq!(
        report.settlements[&OWNER].trade.market_reserved[&material("wood")],
        80_000
    );
    assert_eq!(
        simulation.observe(OWNER).unwrap().trade.limits,
        configuration.economy.trade_limits
    );
}

#[test]
fn agent_construction_uses_configured_cost_and_turns() {
    let mut world = World::new(
        Map::new(3, 1, vec![Area::new(0.5, 0.5); 3], vec![1; 3]),
        vec![Province::new(1)],
    );
    world.set_building_catalog(catalog());
    let mut settlement = Settlement::new(OWNER, 100_000, 1, Arc::default());
    settlement.deposit(material("wood"), 20_000);
    settlement.deposit(material("stone"), 15_000);
    world.add_settlement_at(1, position(0), settlement).unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    simulation
        .queue_decision(SettlementDecision::new(
            OWNER,
            vec![SettlementAction::Construct {
                building_type: BuildingTypeId::new("warehouse"),
                position: position(1),
            }],
        ))
        .unwrap();
    let first = simulation.step().unwrap();
    assert!(first.decisions[0].results[0].is_applied());
    assert_eq!(
        first.settlements[&OWNER].inventory.project_consumed[&material("wood")],
        20_000
    );
    assert_eq!(
        first.settlements[&OWNER].inventory.project_consumed[&material("stone")],
        15_000
    );
    assert_eq!(
        simulation.observe(OWNER).unwrap().inventory.capacity,
        100_000
    );
    simulation.step().unwrap();
    assert_eq!(
        simulation.observe(OWNER).unwrap().inventory.capacity,
        100_000
    );
    simulation.step().unwrap();
    assert_eq!(
        simulation.observe(OWNER).unwrap().inventory.capacity,
        300_000
    );
}
