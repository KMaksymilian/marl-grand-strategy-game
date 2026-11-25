mod common;

use common::material;
use simulation_core::buildings::Building;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition, CycleRecipe};
use simulation_core::economy::ResourceCatalog;
use simulation_core::ids::{BuildingId, NationId, SettlementId};
use simulation_core::nation::decree::DecreeId;
use simulation_core::settlement::happiness::HappinessRules;
use simulation_core::settlement::projects::ProjectCost;
use simulation_core::settlement::{NeedRegistry, Settlement};
use simulation_core::simulation::{NationAction, NationDecision, Simulation};
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::{Map, Position, Province, World, terrain::area::Area};
use std::sync::Arc;

const CAPITAL: SettlementId = SettlementId(1);
const NEIGHBOUR: SettlementId = SettlementId(2);
const RIVAL: SettlementId = SettlementId(3);
const HOME_NATION: NationId = NationId(0);
const RIVAL_NATION: NationId = NationId(1);

fn rules() -> HappinessRules {
    HappinessRules::from_toml(include_str!("../../settlement_happiness.toml")).unwrap()
}

fn needs() -> Arc<NeedRegistry> {
    Arc::new(
        NeedRegistry::from_toml(
            include_str!("../../settlement_needs.toml"),
            ResourceCatalog::standard(),
        )
        .unwrap(),
    )
}

fn world(settlement: Settlement) -> World {
    let mut world = World::new(
        Map::new(4, 1, vec![Area::new(0.5, 0.45); 4], vec![10, 10, 20, 30]),
        [10, 20, 30].into_iter().map(Province::new).collect(),
    );
    world.add_settlement(10, settlement).unwrap();
    world.set_happiness_rules(rules());
    world
}

fn score(world: &World, id: SettlementId) -> f64 {
    world.settlement_happiness(id).unwrap().unwrap().score()
}

fn queue_nation_action(simulation: &mut Simulation, action: NationAction) {
    simulation
        .queue_nation_decision(NationDecision::new(HOME_NATION, vec![action]))
        .unwrap();
}

#[test]
fn gold_at_neutral_happiness_uses_opening_population_even_without_food() {
    let mut settlement = Settlement::new(CAPITAL, 1_000_000, 1_000, needs());
    settlement.deposit_gold(u32::MAX);
    let world = world(settlement);
    assert_eq!(score(&world, CAPITAL), 50.0);
    let mut simulation = Simulation::new(world).unwrap();
    let turn = simulation.step().unwrap();
    let settlement = &turn.settlements[&CAPITAL];
    let happiness = settlement.happiness.as_ref().unwrap();

    assert_eq!(settlement.consumption.satisfaction_percent, 0.0);
    assert_eq!(settlement.consumption.people_unfed, 1_000);
    assert_eq!(happiness.score, 50.0);
    assert_eq!(happiness.gold_income, 1_000);
    let json = serde_json::to_value(happiness).unwrap();
    assert_eq!(json["gold_income"], 1_000);
    assert!(json.get("gold_credited").is_none());
    assert_eq!(happiness.demographics.opening_population, 1_000);
    assert_eq!(happiness.demographics.starved, 100);
    assert_eq!(happiness.demographics.closing_population, 900);
    assert_eq!(simulation.settlement(CAPITAL).unwrap().population(), 900);
    assert_eq!(
        simulation.settlement(CAPITAL).unwrap().gold(),
        u64::from(u32::MAX) + 1_000
    );
    assert_eq!(
        simulation.world().nation(HOME_NATION).unwrap().treasury(),
        0
    );
}

#[test]
fn population_growth_is_capped_by_whole_surplus_rations() {
    let mut settlement = Settlement::new(CAPITAL, 100_000, 200, needs());
    // A population of 200 requires 15,000 grain and 5,000 bread. The remaining
    // 199 grain feed one new person, though 55 happiness allows two.
    for (resource, quantity) in [("grain", 15_199), ("bread", 5_000)] {
        settlement.deposit(material(resource), quantity);
    }
    let mut simulation = Simulation::new(world(settlement)).unwrap();
    queue_nation_action(&mut simulation, NationAction::SetTaxRate { rate: 0.05 });
    let turn = simulation.step().unwrap();
    let report = &turn.settlements[&CAPITAL];
    let happiness = report.happiness.as_ref().unwrap();

    assert_eq!(report.consumption.people_unfed, 0);
    assert_eq!(report.inventory.closing_stock[&material("grain")], 199);
    assert_eq!(happiness.score, 55.0);
    assert_eq!(happiness.demographics.grew, 1);
    assert_eq!(happiness.demographics.closing_population, 201);
    assert_eq!(simulation.settlement(CAPITAL).unwrap().population(), 201);
    assert!(report.inventory.is_balanced());
}

#[test]
fn production_loss_below_thirty_changes_yield_only_in_both_production_models() {
    for cycle_recipe in [false, true] {
        for (weariness, expected_score, expected_loss, expected_output) in
            [(20, 30.0, 0, 100), (21, 29.0, 2_000, 80)]
        {
            let mut settlement = Settlement::new(CAPITAL, 1_000, 25, Arc::default());
            settlement.deposit(material("wood"), 100);
            let mut world = world(settlement);
            world.set_war_weariness(HOME_NATION, weariness).unwrap();
            let legacy_recipe = BuildingBehavior::Production {
                inputs: vec![(material("wood"), 100)],
                outputs: vec![(material("planks"), 100)],
            };
            let behaviors = if cycle_recipe {
                vec![]
            } else {
                vec![legacy_recipe]
            };
            let definition = BuildingDefinition::new(
                "workshop",
                "Workshop",
                3,
                25,
                behaviors,
                ProjectCost::default(),
            )
            .unwrap();
            let definition = if cycle_recipe {
                definition
                    .with_cycle_production(
                        25,
                        vec![CycleRecipe {
                            inputs: vec![(material("wood"), 4)],
                            outputs: vec![(material("planks"), 4)],
                        }],
                    )
                    .unwrap()
            } else {
                definition
            };
            let mut building =
                Building::new(BuildingId(1), Position { x: 1, y: 0 }, Arc::new(definition));
            building.set_target_load(1.0).unwrap();
            world
                .add_building(CAPITAL, building, &mut PathFinder::new(4, 1))
                .unwrap();
            let mut simulation = Simulation::new(world).unwrap();
            let turn = simulation.step().unwrap();
            let report = &turn.settlements[&CAPITAL];
            let happiness = report.happiness.as_ref().unwrap();

            assert_eq!(happiness.score, expected_score);
            assert_eq!(happiness.production_loss_basis_points, expected_loss);
            assert_eq!(
                report.inventory.produced[&material("planks")],
                expected_output
            );
            assert_eq!(report.inventory.production_consumed[&material("wood")], 100);
            assert!(report.inventory.is_balanced());
        }
    }
}

#[test]
fn conquest_cools_down_separately_from_the_nations_capital_loss() {
    let mut world = world(Settlement::new(CAPITAL, 100, 1, Arc::default()));
    world
        .add_settlement_to_nation(
            HOME_NATION,
            20,
            Settlement::new(NEIGHBOUR, 100, 1, Arc::default()),
        )
        .unwrap();
    world
        .add_settlement(30, Settlement::new(RIVAL, 100, 1, Arc::default()))
        .unwrap();
    world.transfer_settlement(CAPITAL, RIVAL_NATION).unwrap();
    assert_eq!(score(&world, CAPITAL), 30.0);
    assert_eq!(score(&world, NEIGHBOUR), 40.0);
    assert_eq!(score(&world, RIVAL), 50.0);

    let mut simulation = Simulation::new(world).unwrap();
    for expected in [30.0, 35.0, 40.0, 45.0, 50.0] {
        let turn = simulation.step().unwrap();
        assert_eq!(
            turn.settlements[&CAPITAL].happiness.as_ref().unwrap().score,
            expected
        );
        assert_eq!(
            turn.settlements[&NEIGHBOUR]
                .happiness
                .as_ref()
                .unwrap()
                .score,
            40.0
        );
    }
}

#[test]
fn an_active_decree_adds_happiness_to_the_score_after_the_base_tax() {
    let mut world = world(Settlement::new(CAPITAL, 100, 1, Arc::default()));
    world.deposit_nation_gold(HOME_NATION, 100).unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    queue_nation_action(&mut simulation, NationAction::SetTaxRate { rate: 0.15 });
    queue_nation_action(
        &mut simulation,
        NationAction::EnactDecree {
            decree: DecreeId::new("games").unwrap(),
        },
    );

    let enacting_turn = simulation.step().unwrap();
    assert_eq!(
        enacting_turn.settlements[&CAPITAL]
            .happiness
            .as_ref()
            .unwrap()
            .score,
        45.0
    );
    let active_turn = simulation.step().unwrap();
    assert_eq!(
        active_turn.settlements[&CAPITAL]
            .happiness
            .as_ref()
            .unwrap()
            .score,
        60.0
    );
}
