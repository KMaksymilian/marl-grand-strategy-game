mod common;

use common::material;
use simulation_core::buildings::{Building, catalog::BuildingCatalog};
use simulation_core::economy::ResourceCatalog;
use simulation_core::ids::{BuildingId, SettlementId};
use simulation_core::settlement::{Settlement, turn_report::InventoryTurnReport};
use simulation_core::simulation::Simulation;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::area::Area;
use simulation_core::world::{Map, Position, Province, World};
use std::sync::Arc;

const SETTLEMENT: SettlementId = SettlementId(1);
const GRASSLAND: Area = Area {
    elevation_val: 0.5,
    moisture_val: 0.45,
};
const FOREST: Area = Area {
    elevation_val: 0.5,
    moisture_val: 0.55,
};

fn catalog() -> BuildingCatalog {
    BuildingCatalog::from_toml(
        include_str!("../../buildings.toml"),
        ResourceCatalog::standard(),
    )
    .unwrap()
}

fn produce(buildings: &[(&str, Area)], stock: &[(&str, u32)]) -> InventoryTurnReport {
    let terrain = std::iter::once(GRASSLAND)
        .chain(buildings.iter().map(|(_, biome)| *biome))
        .collect::<Vec<_>>();
    let width = terrain.len();
    let map = Map::new(width, 1, terrain, vec![1; width]);
    let mut world = World::new(map, vec![Province::new(1)]);
    world.set_building_catalog(catalog());
    let mut settlement = Settlement::new(
        SETTLEMENT,
        10_000,
        25 * buildings.len() as u32,
        Arc::default(),
    );
    for &(id, quantity) in stock {
        settlement.deposit(material(id), quantity);
    }
    world
        .add_settlement_at(1, Position { x: 0, y: 0 }, settlement)
        .unwrap();

    let mut finder = PathFinder::new(width, 1);
    for (index, (id, _)) in buildings.iter().enumerate() {
        let definition = world.building_catalog().get_blueprint(id).unwrap();
        let mut building = Building::new(
            BuildingId(index as u32),
            Position {
                x: (index + 1) as i32,
                y: 0,
            },
            definition,
        );
        building.set_target_load(1.0).unwrap();
        world
            .add_building(SETTLEMENT, building, &mut finder)
            .unwrap();
    }

    let mut simulation = Simulation::new(world).unwrap();
    let report = simulation
        .step()
        .unwrap()
        .settlements
        .remove(&SETTLEMENT)
        .unwrap()
        .inventory;
    assert!(report.is_balanced());
    report
}

#[test]
fn economic_blueprints_load_with_tier_defaults_and_deposit_variants() {
    let catalog = catalog();
    let mut counts = [0; 3];
    for id in catalog.ids() {
        let blueprint = catalog.get_blueprint(id).unwrap();
        let Some(production) = blueprint.cycle_production() else {
            continue;
        };
        let tier_index = usize::from(blueprint.tier() - 1);
        counts[tier_index] += 1;
        assert_eq!(blueprint.max_workers(), 25, "{id}");
        assert_eq!(
            blueprint.construction_cost().turn_cost,
            [3, 5, 8][tier_index],
            "{id}"
        );
        let rate = if id == "charcoal_kiln" {
            50
        } else {
            [100, 50, 25][tier_index]
        };
        assert_eq!(production.cycles_per_turn, rate, "{id}");
    }
    assert_eq!(counts, [13, 10, 4]);

    let plantation = catalog.get_blueprint("plantation").unwrap();
    assert_eq!(
        plantation.construction_cost().needed_resources[&material("wood")],
        200
    );
    let mine = catalog.get_blueprint("mine_iron_ore").unwrap();
    assert_eq!(mine.required_deposit(), Some(material("iron_ore")));
    assert_eq!(
        mine.cycle_production().unwrap().recipes[0].outputs,
        [(material("iron_ore"), 1)]
    );
}

#[test]
fn plantations_use_their_tile_efficiency_and_production_totals_add_up() {
    for (sites, expected) in [
        (vec![("plantation", GRASSLAND)], 100),
        (vec![("plantation", FOREST)], 75),
        (vec![("plantation", GRASSLAND), ("plantation", FOREST)], 175),
    ] {
        let report = produce(&sites, &[]);
        assert_eq!(report.produced[&material("grain")], expected);
        assert!(report.production_consumed.is_empty());
    }
}

#[test]
fn sawmill_and_steelworks_execute_the_worked_examples() {
    let report = produce(
        &[("sawmill", GRASSLAND), ("steelworks", FOREST)],
        &[("wood", 150), ("coal", 50), ("iron_ore", 50)],
    );
    assert_eq!(report.produced[&material("planks")], 100);
    assert_eq!(report.produced[&material("steel")], 50);
    for (input, expected) in [("wood", 150), ("coal", 50), ("iron_ore", 50)] {
        assert_eq!(report.production_consumed[&material(input)], expected);
    }
}

#[test]
fn charcoal_kiln_respects_full_cycles_and_its_processing_rate() {
    for (wood, expected_coal, wood_remaining) in [(99, 49, 1), (102, 50, 2)] {
        let report = produce(&[("charcoal_kiln", GRASSLAND)], &[("wood", wood)]);
        assert_eq!(report.produced[&material("coal")], expected_coal);
        assert_eq!(
            report.production_consumed[&material("wood")],
            2 * expected_coal
        );
        assert_eq!(report.closing_stock[&material("wood")], wood_remaining);
    }
}

#[test]
fn builder_selects_one_recipe_from_available_inputs_in_configuration_order() {
    for (stock, expected_usage, expected_outputs) in [
        ((50, 50), (50, 0), 25),  // All tie at 25 cycles.
        ((0, 50), (0, 50), 25),   // Only the brick recipe is feasible.
        ((25, 25), (25, 25), 25), // The mixed recipe is best.
        ((1, 1), (1, 1), 1),      // Neither single-input recipe can run.
        ((1, 0), (0, 0), 0),      // No complete input bundle.
    ] {
        let report = produce(
            &[("builder", GRASSLAND)],
            &[("stone_bricks", stock.0), ("bricks", stock.1)],
        );
        let consumed = |id| {
            report
                .production_consumed
                .get(&material(id))
                .copied()
                .unwrap_or(0)
        };
        let output = report
            .produced
            .get(&material("fortification_components"))
            .copied()
            .unwrap_or(0);
        assert_eq!(
            (consumed("stone_bricks"), consumed("bricks")),
            expected_usage
        );
        assert_eq!(output, expected_outputs);
    }
}

#[test]
fn builders_plan_against_the_same_stock_without_requesting_the_same_bundle_twice() {
    let report = produce(
        &[("builder", GRASSLAND), ("builder", GRASSLAND)],
        &[("stone_bricks", 50), ("bricks", 50)],
    );
    assert_eq!(report.produced[&material("fortification_components")], 50);
    assert_eq!(report.production_consumed[&material("stone_bricks")], 50);
    assert_eq!(report.production_consumed[&material("bricks")], 50);
}
