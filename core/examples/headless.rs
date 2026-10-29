//! Run with: cargo run -p simulation_core --example headless -- 100
use simulation_core::ids::SettlementId;
use std::collections::HashMap;
use std::sync::Arc;

use simulation_core::buildings::{
    Building,
    definition::{BuildingBehavior, BuildingDefinition},
};
use simulation_core::economy::ResourceType::Grain;
use simulation_core::ids::BuildingId;
use simulation_core::settlement::{NeedRegistry, NeedTier, Settlement, projects::ProjectCost};
use simulation_core::simulation::Simulation;
use simulation_core::world::{
    Map, Position, Province, World, pathfinding::path_finder::PathFinder, terrain::area::Area,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let turns = std::env::args()
        .nth(1)
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(10);
    let mut world = World::new(
        Map::new(1, 1, vec![Area::new(0.5, 0.5)], vec![0]),
        vec![Province::new(0)],
    );
    let needs = Arc::new(NeedRegistry::new(vec![NeedTier {
        min_population: 1,
        needs_per_capita: HashMap::from([(Grain, 1)]),
    }]));
    let mut settlement = Settlement::new(SettlementId(1), 100, 1, needs);
    settlement.deposit(Grain, 1);
    world
        .add_settlement(0, settlement)
        .expect("valid example settlement");
    let definition = Arc::new(BuildingDefinition {
        id: "farm".into(),
        name: "Farm".into(),
        tier: 1,
        max_workers: 1,
        behaviors: vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(Grain, 2)],
        }],
        construction_cost: ProjectCost::default(),
    });
    let mut building = Building::new(BuildingId(1), Position { x: 0, y: 0 }, definition)
        .expect("valid example building");
    building.set_target_load(1.0).expect("valid load");
    world
        .add_building(SettlementId(1), building, &mut PathFinder::new(1, 1))
        .expect("valid example placement");
    let mut simulation = Simulation::new(world)?;
    for _ in 0..turns {
        simulation.step()?;
    }
    println!(
        "Completed {} turns; settlement 1 grain: {}",
        simulation.current_turn(),
        simulation
            .world()
            .settlement(SettlementId(1))
            .expect("registered settlement")
            .inventory()
            .amount(Grain)
    );
    Ok(())
}
