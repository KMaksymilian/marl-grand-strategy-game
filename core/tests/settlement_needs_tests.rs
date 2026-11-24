mod common;

use common::material;
use simulation_core::economy::ResourceCatalog;
use simulation_core::ids::SettlementId;
use simulation_core::settlement::{NeedRegistry, Settlement, start::SettlementStart};
use simulation_core::simulation::Simulation;
use simulation_core::world::terrain::area::Area;
use simulation_core::world::{Map, Province, World};
use std::sync::Arc;

fn needs() -> Arc<NeedRegistry> {
    Arc::new(
        NeedRegistry::from_toml(
            include_str!("../../settlement_needs.toml"),
            ResourceCatalog::standard(),
        )
        .unwrap(),
    )
}

fn world() -> World {
    World::new(
        Map::new(2, 1, vec![Area::new(0.5, 0.5); 2], vec![1; 2]),
        vec![Province::new(1)],
    )
}

#[test]
fn founding_loads_the_exact_five_turn_diet_and_validates_storage_capacity() {
    let resources = ResourceCatalog::standard();
    let profile = SettlementStart::from_toml(
        include_str!("../../settlement_start.toml"),
        resources,
        needs(),
    )
    .unwrap();
    let mut world = world();
    let id = world.found_settlement(1, None, &profile).unwrap();
    let settlement = world.settlement(id).unwrap();
    assert_eq!(settlement.population(), 1_000);
    assert_eq!(settlement.inventory().capacity(), 1_000_000);
    assert_eq!(settlement.inventory().amount(material("wood")), 60_000);
    assert_eq!(settlement.inventory().amount(material("stone")), 30_000);
    let food = needs().starting_food(1_000);
    assert_eq!(food.values().sum::<u32>(), 100_000);
    assert_eq!(food[&material("grain")], 35_000);
    for (resource, units) in food {
        assert_eq!(settlement.inventory().amount(resource), units * 5);
    }
    assert_eq!(settlement.inventory().total_amount(), 590_000);
    assert!(SettlementStart::from_toml(
        "population = 1000\nwarehouse_capacity = 5899\nfood_turns = 5\nmaterials = { wood = 600, stone = 300 }",
        resources, needs(),
    ).is_err());
}

#[test]
fn tier_boundaries_replace_diets_and_accumulate_material_needs() {
    let registry = needs();
    for (population, expected) in [
        (0, 0),
        (1, 1),
        (99, 1),
        (100, 2),
        (249, 2),
        (250, 3),
        (499, 3),
        (500, 4),
        (799, 4),
        (800, 5),
    ] {
        assert_eq!(registry.get_active_tiers(population).count(), expected);
    }
    let settlement = Settlement::new(SettlementId(1), 100_000, 120, registry.clone());
    let demand = settlement.needs_per_turn();
    assert_eq!(demand[&material("grain")], 9_000);
    assert_eq!(demand[&material("bread")], 3_000);
    assert_eq!(demand[&material("wood")], 2_400);
    assert_eq!(demand[&material("beer")], 600);
    let higher = Settlement::new(SettlementId(2), 100_000, 800, registry);
    let demand = higher.needs_per_turn();
    for (resource, units) in [
        ("wood", 16_000),
        ("beer", 4_000),
        ("tools", 4_000),
        ("furniture", 2_400),
    ] {
        assert_eq!(demand[&material(resource)], units);
    }
    assert!(!demand.contains_key(&material("monument_components")));
}

#[test]
fn food_substitutions_use_actual_stock_and_count_only_uncovered_rations() {
    for (extra_fish, food_eaten, unfed) in [(5_000, 12_000, 0), (4_850, 11_850, 2)] {
        let id = SettlementId(1);
        let mut settlement = Settlement::new(id, 100_000, 120, needs());
        for (resource, units) in [
            ("grain", 6_000),
            ("bread", 1_000),
            ("fish", extra_fish),
            ("wood", 2_400),
        ] {
            settlement.deposit(material(resource), units);
        }
        let mut world = world();
        world.add_settlement(1, settlement).unwrap();
        let mut simulation = Simulation::new(world).unwrap();
        let turn = simulation.step().unwrap();
        let report = &turn.settlements[&id];
        assert_eq!(report.consumption.food_required, 12_000);
        assert_eq!(report.consumption.food_eaten, food_eaten);
        assert_eq!(report.consumption.people_unfed, unfed);
        assert!(
            report
                .consumption
                .missing_resources
                .contains(&material("beer"))
        );
        assert_eq!(
            report.inventory.population_consumed[&material("fish")],
            u64::from(extra_fish)
        );
        assert!(report.inventory.is_balanced());
        assert_eq!(simulation.observe(id).unwrap().workforce.available, 120);
    }
}
