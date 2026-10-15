use adapters::view::map::game_screen::GameScreen;
use adapters::view::map::view_generator::ViewGenerator;
use core::configuration::Configuration;
use core::domain::world_data::world::World;
use core::services::algorithms::path_finder::PathFinder;
use core::services::world_generation::world_builder::WorldBuilder;
use macroquad::prelude::*;

#[macroquad::main("Grand Strategy v1.0")]
async fn main() {
    let configuration = Configuration::load("configuration.toml")
        .unwrap_or_else(|error| panic!("configuration.toml: {error}"));
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
    let world: World = WorldBuilder::generate_default();
    let mut game_screen: GameScreen = ViewGenerator::generate(&world);
    let mut path_finder: PathFinder = PathFinder::new(world.map.width, world.map.height);

    loop {
        clear_background(LIGHTGRAY);
        game_screen.run(&world, &mut path_finder);
        next_frame().await
    }
}
