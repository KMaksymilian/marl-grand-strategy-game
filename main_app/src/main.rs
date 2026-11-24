use adapters::view::map::game_screen::GameScreen;
use adapters::view::map::view_generator::ViewGenerator;
use macroquad::prelude::*;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::configuration::Configuration;
use simulation_core::economy::ResourceCatalog;
use simulation_core::military::UnitCatalog;
use simulation_core::nation::decree_catalog::DecreeCatalog;
use simulation_core::settlement::happiness::HappinessRules;
use simulation_core::settlement::{NeedRegistry, start::SettlementStart};
use simulation_core::world::World;
use simulation_core::world::generation::world_builder::WorldBuilder;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use std::sync::Arc;

#[macroquad::main("Grand Strategy v1.0")]
async fn main() {
    let configuration = Configuration::load("configuration.toml")
        .unwrap_or_else(|error| panic!("configuration.toml: {error}"));
    let resources = ResourceCatalog::load("resources.toml")
        .unwrap_or_else(|error| panic!("resources.toml: {error}"));
    info!("Loaded {} materials", resources.materials().len());
    let needs = Arc::new(
        NeedRegistry::load("settlement_needs.toml", &resources)
            .unwrap_or_else(|error| panic!("settlement_needs.toml: {error}")),
    );
    let start = SettlementStart::load("settlement_start.toml", &resources, needs)
        .unwrap_or_else(|error| panic!("settlement_start.toml: {error}"));
    let buildings = BuildingCatalog::load("buildings.toml", &resources)
        .unwrap_or_else(|error| panic!("buildings.toml: {error}"));
    info!("Loaded {} building types", buildings.ids().len());
    let units = UnitCatalog::load("units.toml", &resources)
        .unwrap_or_else(|error| panic!("units.toml: {error}"));
    info!("Loaded {} unit types", units.definitions().len());
    let decrees =
        DecreeCatalog::load("decrees.toml").unwrap_or_else(|error| panic!("decrees.toml: {error}"));
    info!("Loaded {} decrees", decrees.definitions().len());
    info!(
        "Building productivity uses A* costs: full up to {}, zero from {}",
        configuration
            .economy
            .building_productivity
            .full_productivity_cost(),
        configuration
            .economy
            .building_productivity
            .zero_productivity_cost()
    );
    let mut world: World = WorldBuilder::generate_default_with_resources(Arc::new(resources));
    world.set_happiness_rules(
        HappinessRules::load("settlement_happiness.toml")
            .unwrap_or_else(|error| panic!("settlement_happiness.toml: {error}")),
    );
    world.set_building_catalog(buildings);
    configuration.configure_world(&mut world);
    let initial_province = world
        .map()
        .terrain
        .iter()
        .zip(&world.map().territory)
        .find(|(area, province)| {
            area.determine_biome() != simulation_core::world::terrain::biome::Biome::Ocean
                && world.province(**province).is_ok()
        })
        .map(|(_, &province)| province)
        .expect("generated world has a land province");
    let founder = world
        .found_settlement(initial_province, None, &start)
        .expect("generated land province supports a settlement");
    info!("Founded initial settlement {:?}", founder);
    world
        .set_unit_catalog(Arc::new(units))
        .unwrap_or_else(|error| panic!("units.toml: {error:?}"));
    world
        .set_decree_catalog(Arc::new(decrees))
        .expect("a new world has no decrees yet");
    let mut game_screen: GameScreen = ViewGenerator::generate(&world);
    let mut path_finder: PathFinder = PathFinder::new(world.map().width, world.map().height);

    loop {
        clear_background(LIGHTGRAY);
        game_screen.run(&world, &mut path_finder);
        next_frame().await
    }
}
