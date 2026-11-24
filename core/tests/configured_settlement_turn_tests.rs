mod common;

use common::material;
use simulation_core::buildings::{
    Building,
    catalog::BuildingCatalog,
    definition::{BuildingBehavior, BuildingDefinition},
};
use simulation_core::configuration::Configuration;
use simulation_core::economy::ResourceCatalog;
use simulation_core::ids::{BuildingId, BuildingTypeId, NationId, SettlementId};
use simulation_core::military::UnitCatalog;
use simulation_core::nation::{decree::DecreeId, decree_catalog::DecreeCatalog};
use simulation_core::settlement::{
    NeedRegistry, Settlement,
    decision::{SettlementAction, SettlementDecision},
    happiness::HappinessRules,
    start::SettlementStart,
};
use simulation_core::simulation::{NationAction, NationDecision, Simulation};
use simulation_core::world::{
    Map, Position, Province, World, generation::deposits::DepositGenerationConfig,
    pathfinding::path_finder::PathFinder, terrain::area::Area,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

fn content(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(file)
}

fn configured_world() -> (World, Arc<NeedRegistry>) {
    let resources = Arc::new(ResourceCatalog::load(content("resources.toml")).unwrap());
    let needs = Arc::new(NeedRegistry::load(content("settlement_needs.toml"), &resources).unwrap());
    let mut terrain = vec![Area::new(0.5, 0.45); 32];
    terrain[31] = Area::new(0.5, 0.7);
    let mut world = World::with_resource_catalog(
        Map::new(32, 1, terrain, vec![1; 32]),
        vec![Province::new(1)],
        resources.clone(),
    );
    world
        .generate_deposits(&DepositGenerationConfig::for_catalog(67, &resources))
        .unwrap();
    Configuration::load(content("configuration.toml"))
        .unwrap()
        .configure_world(&mut world);
    world.set_building_catalog(
        BuildingCatalog::load(content("buildings.toml"), &resources).unwrap(),
    );
    world
        .set_unit_catalog(Arc::new(
            UnitCatalog::load(content("units.toml"), &resources).unwrap(),
        ))
        .unwrap();
    world
        .set_decree_catalog(Arc::new(
            DecreeCatalog::load(content("decrees.toml")).unwrap(),
        ))
        .unwrap();
    world.set_happiness_rules(HappinessRules::load(content("settlement_happiness.toml")).unwrap());
    (world, needs)
}

fn position(x: i32) -> Position {
    Position { x, y: 0 }
}

fn add_staffed(
    world: &mut World,
    settlement: SettlementId,
    id: u32,
    x: i32,
    blueprint: Arc<BuildingDefinition>,
) {
    let mut building = Building::new(BuildingId(id), position(x), blueprint);
    building.set_target_load(1.0).unwrap();
    world
        .add_building(settlement, building, &mut PathFinder::new(32, 1))
        .unwrap();
}

fn enact(simulation: &mut Simulation, id: &str) {
    simulation
        .queue_nation_decision(NationDecision::new(
            NationId(0),
            vec![NationAction::EnactDecree {
                decree: DecreeId::new(id).unwrap(),
            }],
        ))
        .unwrap();
}

#[test]
fn configured_game_foundation_production_and_warehouse_lifecycle_remain_balanced() {
    let (mut world, needs) = configured_world();
    let start = SettlementStart::load(
        content("settlement_start.toml"),
        world.resource_catalog(),
        needs,
    )
    .unwrap();
    let settlement = world
        .found_settlement(1, Some(position(0)), &start)
        .unwrap();
    let plantation = world
        .building_catalog()
        .get_blueprint("plantation")
        .unwrap();
    for (id, x) in [(0, 1), (1, 20), (2, 30)] {
        add_staffed(&mut world, settlement, id, x, plantation.clone());
    }
    let mut simulation = Simulation::new(world).unwrap();
    simulation
        .queue_decision(SettlementDecision::new(
            settlement,
            vec![SettlementAction::Construct {
                building_type: BuildingTypeId::new("warehouse"),
                position: position(2),
            }],
        ))
        .unwrap();

    for turn in 1..=5 {
        if turn == 4 {
            simulation
                .queue_decision(SettlementDecision::new(
                    settlement,
                    vec![SettlementAction::Demolish {
                        building_id: BuildingId(3),
                    }],
                ))
                .unwrap();
        }
        let report = simulation.step().unwrap();
        let state = simulation.observe(settlement).unwrap();
        let local = &report.settlements[&settlement];
        let capacity = if turn == 3 { 1_200_000 } else { 1_000_000 };
        assert_eq!(state.inventory.capacity, capacity, "turn {turn}");
        assert_eq!(state.population, 1_000, "turn {turn}");
        assert_eq!(state.gold, turn * 1_000);
        assert_eq!(local.consumption.food_required, 100_000);
        assert_eq!(local.consumption.people_unfed, 0);
        // Route costs 10, 200 and 300 yield 100%, 50% and 0% under the real TOML.
        assert_eq!(local.inventory.produced[&material("grain")], 15_000);
        assert!(local.inventory.is_balanced(), "turn {turn}");
        if matches!(turn, 1 | 4) {
            assert!(report.decisions[0].results[0].is_applied());
        }
    }
}

#[test]
fn an_active_production_decree_matches_each_output_material() {
    let (mut world, _) = configured_world();
    let settlement = SettlementId(5);
    world
        .add_settlement_at(
            1,
            position(0),
            Settlement::new(settlement, 1_000_000, 120, Arc::default()),
        )
        .unwrap();
    assert_eq!(world.deposit_nation_gold(NationId(0), 100).unwrap(), 100);
    let plantation = world
        .building_catalog()
        .get_blueprint("plantation")
        .unwrap();
    add_staffed(&mut world, settlement, 0, 1, plantation);
    let mixed_output = Arc::new(
        BuildingDefinition::new(
            "mixed",
            "Mixed outputs",
            1,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![
                    (material("grain"), 101),
                    (material("wood"), 101),
                    (material("tools"), 101),
                ],
            }],
            Default::default(),
        )
        .unwrap(),
    );
    add_staffed(&mut world, settlement, 1, 2, mixed_output);
    let mut simulation = Simulation::new(world).unwrap();
    enact(&mut simulation, "agricultural_decree");

    for (grain, wood) in [(10_101, 101), (12_121, 90)] {
        let turn = simulation.step().unwrap();
        let inventory = &turn.settlements[&settlement].inventory;
        assert_eq!(inventory.produced[&material("grain")], grain);
        assert_eq!(inventory.produced[&material("wood")], wood);
        assert_eq!(inventory.produced[&material("tools")], 101);
        assert!(inventory.is_balanced());
    }
}

#[test]
fn one_rationing_plan_is_used_for_demand_substitutions_and_hunger() {
    let (mut world, needs) = configured_world();
    let id = SettlementId(5);
    let mut settlement = Settlement::new(id, 1_000_000, 120, needs);
    settlement.deposit(material("fish"), 20_500);
    settlement.deposit(material("wood"), 4_800);
    world.add_settlement_at(1, position(0), settlement).unwrap();
    assert_eq!(world.deposit_nation_gold(NationId(0), 100).unwrap(), 100);
    let mut simulation = Simulation::new(world).unwrap();
    enact(&mut simulation, "food_rationing");

    for (required, eaten, unfed) in [(12_000, 12_000, 0), (9_000, 8_500, 7)] {
        let turn = simulation.step().unwrap();
        let report = &turn.settlements[&id];
        assert_eq!(report.consumption.food_required, required);
        assert_eq!(report.consumption.food_eaten, eaten);
        assert_eq!(report.consumption.people_unfed, unfed);
        assert_eq!(
            report.inventory.population_consumed[&material("fish")],
            eaten
        );
        assert_eq!(
            report.inventory.population_consumed[&material("wood")],
            2_400
        );
        assert!(report.inventory.is_balanced());
        if required == 12_000 {
            let next_demand = simulation.observe(id).unwrap().needs_per_turn;
            assert_eq!(next_demand[&material("grain")], 6_750);
            assert_eq!(next_demand[&material("bread")], 2_250);
        }
    }
}
