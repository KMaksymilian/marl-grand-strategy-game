use core::domain::world_data::world::World;
use core::services::world_generation::procedural_generator::{
    ElevationTuningFineConfig, ProceduralWorldGenerator, WorldGenerationConfig,
};

use adapters::view::map::game_screen::GameScreen;
use adapters::view::map::map_camera::MapCamera;
use adapters::view::map::map_view::MapView;
use macroquad::prelude::*;

#[macroquad::main("Grand Strategy v1.0")]
async fn main() {
    let world_generation_config: WorldGenerationConfig = WorldGenerationConfig {
        width: 2000,
        height: 1500,
        province_count: 500,
        iteration_count: 3,
        elevation_scale: 200.0,
        moisture_scale: 600.0,
        min_max: (0.0, 1.0),
        warp_scale: 50.0,
        warp_intensity: 12.5,
        points_seed_option: Some(67),
        borders_seed_option: Some(69),
        elevation_seed_option: Some(420),
        moisture_seed_option: Some(2137),
    };

    let elevation_tuning_fine_config: ElevationTuningFineConfig = ElevationTuningFineConfig {
        distance_multiplyer: 0.8,
        dropoff_powi: 4,
        dropoff_multiplyer: 1.5,
        final_val_addition: 0.25,
    };

    let world: World =
        ProceduralWorldGenerator::generate(&world_generation_config, &elevation_tuning_fine_config);

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
