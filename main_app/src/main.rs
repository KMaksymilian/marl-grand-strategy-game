use core::domain::world_data::world::World;
use core::services::world_generation::procedural_generator::{
    ProceduralWorldGenerator, WorldGenerationConfig,
};

use adapters::view::map::game_screen::GameScreen;
use adapters::view::map::map_camera::MapCamera;
use adapters::view::map::map_view::MapView;
use macroquad::prelude::*;

#[macroquad::main("Grand Strategy v1.0")]
async fn main() {
    let world_generation_config: WorldGenerationConfig = WorldGenerationConfig {
        width: 1500,
        height: 1500,
        province_count: 450,
        iteration_count: 3,
        elevation_scale: 200.0,
        moisture_scale: 600.0,
        min_max: (0.0, 1.0),
        warp_scale: 50.0,
        warp_intensity: 15.0,
        borders_seed_option: None,   // Some(67),
        elevation_seed_option: None, //Some(420),
        moisture_seed_option: None,  //Some(2137),
    };
    let world: World = ProceduralWorldGenerator::generate(&world_generation_config);

    let map_view: MapView = MapView::generate_from_world(&world);
    let map_camera: MapCamera =
        MapCamera::new(0.02, world.map.width as f32, world.map.height as f32);
    let mut game_screen: GameScreen = GameScreen::new(map_view, map_camera);

    loop {
        clear_background(LIGHTGRAY);
        game_screen.run();
        next_frame().await
    }
}
