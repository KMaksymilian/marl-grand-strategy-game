mod common;

use common::material;
use simulation_core::buildings::Building;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::buildings::productivity::{
    BiomeProductionModifier, BuildingProductivityCalculator, ProductionFactor, ProductionModifier,
    ProductionModifierContext, ProductionModifierStrategy, TravelCostEfficiencyPolicy,
};
use simulation_core::ids::{BuildingId, SettlementId};
use simulation_core::settlement::{Settlement, projects::ProjectCost};
use simulation_core::simulation::Simulation;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::settlement_environment::AStarTravelCosts;
use simulation_core::world::terrain::{area::Area, biome::Biome};
use simulation_core::world::{Map, Position, Province, World};
use std::sync::Arc;

const SETTLEMENT: SettlementId = SettlementId(1);
const WORKSHOP: BuildingId = BuildingId(1);
const SITE: Position = Position { x: 2, y: 0 };
const CAPITAL: Position = Position { x: 0, y: 0 };

#[derive(Debug)]
struct WorkshopDecree(i32);

impl ProductionModifierStrategy for WorkshopDecree {
    fn modifier(&self, context: &ProductionModifierContext<'_>) -> Option<ProductionModifier> {
        (context.building.instance_id() == WORKSHOP && context.building.current_workers() == 4)
            .then(|| ProductionModifier::new("workshop decree", self.0))
    }
}

fn scenario(
    terrain: [Area; 3],
    policy: Option<TravelCostEfficiencyPolicy>,
    base_output: u32,
    configure: impl FnOnce(&mut Settlement),
) -> Simulation {
    let map = Map::new(3, 1, terrain.to_vec(), vec![1; 3]);
    let mut finder = PathFinder::new(3, 1);
    let mut settlement = Settlement::new(SETTLEMENT, 1000, 4, Arc::default());
    settlement.deposit(material("wood"), 3);
    settlement.deposit(material("stone"), 4);
    if let Some(policy) = policy {
        settlement.set_building_productivity_calculator(
            Some(BuildingProductivityCalculator::new(CAPITAL, policy)),
            &mut AStarTravelCosts::new(&map, &mut finder),
        );
    }
    configure(&mut settlement);

    let definition = BuildingDefinition::new(
        "workshop",
        "Workshop",
        2,
        4,
        vec![BuildingBehavior::Production {
            inputs: vec![(material("wood"), 4), (material("stone"), 4)],
            outputs: vec![(material("planks"), base_output)],
        }],
        ProjectCost::default(),
    )
    .unwrap();
    let mut building = Building::new(WORKSHOP, SITE, Arc::new(definition));
    building.set_target_load(1.0).unwrap();
    let mut world = World::new(map, vec![Province::new(1)]);
    world.add_settlement_at(1, CAPITAL, settlement).unwrap();
    world
        .add_building(SETTLEMENT, building, &mut finder)
        .unwrap();
    Simulation::new(world).unwrap()
}

#[test]
fn biome_and_decree_stack_after_allocation_without_changing_recipe_cost() {
    let grassland = Area::new(0.5, 0.45);
    let forest = Area::new(0.5, 0.55);
    let mut simulation = scenario([grassland, grassland, forest], None, 101, |settlement| {
        settlement.add_production_modifier_strategy(BiomeProductionModifier::new(
            "workshop",
            Biome::Forest,
            -2500,
        ));
        settlement.add_production_modifier_strategy(WorkshopDecree(5000));
    });
    let turn = simulation.step().unwrap();
    let inventory = &turn.settlements[&SETTLEMENT].inventory;

    // Wood limits the recipe to 3/4. Floor only after applying the net +25%.
    // Flooring base output first would give 93, multiplying modifiers gives 85.
    assert_eq!(inventory.produced[&material("planks")], 94);
    assert_eq!(inventory.production_consumed[&material("wood")], 3);
    assert_eq!(inventory.production_consumed[&material("stone")], 3);
    assert_eq!(inventory.returned[&material("stone")], 1);
    assert!(inventory.is_balanced());
}

#[test]
fn yield_operations_use_registration_order_and_allow_a_custom_cap() {
    let add = || ProductionModifier::new("decree", 2_000);
    let multiply = || ProductionModifier::multiplicative("tools", 15_000);
    let cap = ProductionModifier::with_operation("cap", |factor| {
        factor.min(ProductionFactor::new(5.into(), 4.into()))
    });
    let grassland = Area::new(0.5, 0.45);

    for (modifiers, expected_planks) in [
        (vec![add(), multiply()], 136),
        (vec![multiply(), add()], 128),
        (vec![add(), multiply(), cap], 94),
    ] {
        let mut simulation = scenario([grassland; 3], None, 101, |settlement| {
            for modifier in modifiers {
                settlement.add_production_modifier_strategy(modifier);
            }
        });
        let turn = simulation.step().unwrap();
        let inventory = &turn.settlements[&SETTLEMENT].inventory;
        assert_eq!(inventory.produced[&material("planks")], expected_planks);
        assert_eq!(inventory.production_consumed[&material("wood")], 3);
        assert_eq!(inventory.production_consumed[&material("stone")], 3);
        assert!(inventory.is_balanced());
    }
}

#[test]
fn reachable_astar_penalty_stacks_with_decree() {
    let grassland = Area::new(0.5, 0.45);
    let mut simulation = scenario(
        [grassland; 3],
        Some(TravelCostEfficiencyPolicy::new(10, 50).unwrap()),
        101,
        |settlement| settlement.add_production_modifier_strategy(WorkshopDecree(2000)),
    );
    let turn = simulation.step().unwrap();
    let inventory = &turn.settlements[&SETTLEMENT].inventory;

    // Travel cost 20 yields -25%. The decree makes the combined factor 95%.
    assert_eq!(inventory.produced[&material("planks")], 71);
    assert_eq!(inventory.production_consumed[&material("wood")], 3);
    assert!(inventory.is_balanced());
}

#[test]
fn a_total_penalty_below_minus_one_hundred_percent_clamps_output_to_zero() {
    let forest = Area::new(0.5, 0.55);
    let mut simulation = scenario([forest; 3], None, 101, |settlement| {
        settlement.add_production_modifier_strategy(BiomeProductionModifier::new(
            "workshop",
            Biome::Forest,
            -2500,
        ));
        settlement.add_production_modifier_strategy(WorkshopDecree(-8500));
    });
    let turn = simulation.step().unwrap();
    let inventory = &turn.settlements[&SETTLEMENT].inventory;

    assert!(!inventory.produced.contains_key(&material("planks")));
    assert_eq!(inventory.production_consumed[&material("wood")], 3);
    assert_eq!(inventory.production_consumed[&material("stone")], 3);
    assert!(inventory.is_balanced());
}

#[test]
fn a_decree_cannot_restore_production_across_an_unreachable_route() {
    let grassland = Area::new(0.5, 0.45);
    let ocean = Area::new(0.0, 0.45);
    let mut simulation = scenario(
        [grassland, ocean, grassland],
        Some(TravelCostEfficiencyPolicy::new(10, 50).unwrap()),
        101,
        |settlement| settlement.add_production_modifier_strategy(WorkshopDecree(5000)),
    );
    let turn = simulation.step().unwrap();
    let inventory = &turn.settlements[&SETTLEMENT].inventory;

    assert!(!inventory.produced.contains_key(&material("planks")));
    assert_eq!(inventory.production_consumed[&material("wood")], 3);
    assert!(inventory.is_balanced());
}

#[test]
fn bonus_output_above_u32_capacity_is_reported_before_storage_losses() {
    let grassland = Area::new(0.5, 0.45);
    let mut simulation = scenario([grassland; 3], None, u32::MAX, |settlement| {
        settlement.deposit(material("wood"), 1);
        settlement.add_production_modifier_strategy(WorkshopDecree(1000));
    });
    let turn = simulation.step().unwrap();
    let inventory = &turn.settlements[&SETTLEMENT].inventory;
    let planks = material("planks");
    let expected = u64::from(u32::MAX) * 11 / 10;

    assert_eq!(inventory.produced[&planks], expected);
    assert_eq!(inventory.closing_stock[&planks], 1000);
    let lost_planks: u64 = inventory
        .losses
        .iter()
        .filter(|loss| loss.resource == planks)
        .map(|loss| loss.amount)
        .sum();
    assert_eq!(
        inventory.closing_stock[&planks] + inventory.taxed[&planks] + lost_planks,
        expected
    );
    assert!(inventory.is_balanced());
}
