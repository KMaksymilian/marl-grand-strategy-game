use adapters::view::map::game_screen::GameScreen;
use adapters::view::map::view_generator::ViewGenerator;
use core::domain::world_data::world::World;
use core::services::world_generation::world_generator::WorldGenerator;
use macroquad::prelude::*;

#[macroquad::main("Grand Strategy v1.0")]
async fn main() {
    let world: World = WorldGenerator::generate();
    let mut game_screen: GameScreen = ViewGenerator::generate(&world);

    loop {
        clear_background(LIGHTGRAY);
        game_screen.run();
        next_frame().await
    }
}
