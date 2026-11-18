use adapters::view::map::game_screen::GameScreen;
use adapters::view::map::view_generator::ViewGenerator;
use macroquad::prelude::*;
use simulation_core::configuration::Configuration;
use simulation_core::economy::ResourceCatalog;
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
    let world: World = WorldBuilder::generate_default_with_resources(Arc::new(resources));
    let mut game_screen: GameScreen = ViewGenerator::generate(&world);
    let mut path_finder: PathFinder = PathFinder::new(world.map().width, world.map().height);

    loop {
        clear_background(LIGHTGRAY);
        game_screen.run(&world, &mut path_finder);
        next_frame().await
    }
}
