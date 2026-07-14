use crate::{
    domain::world_data::world::World,
    services::world_generation::procedural_generator::{
        ElevationTuningFineConfig, ProceduralWorldGenerator, WorldGenerationConfig,
    },
};

pub struct WorldGenerator;

impl WorldGenerator {
    pub fn generate() -> World {
        let world_generation_config: WorldGenerationConfig =
            WorldGenerator::world_generation_config();
        let elevation_tuning_fine_config: ElevationTuningFineConfig =
            WorldGenerator::elevation_tuning_fine_tuning();
        ProceduralWorldGenerator::generate(&world_generation_config, &elevation_tuning_fine_config)
    }

    fn world_generation_config() -> WorldGenerationConfig {
        WorldGenerationConfig {
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
        }
    }

    fn elevation_tuning_fine_tuning() -> ElevationTuningFineConfig {
        ElevationTuningFineConfig {
            distance_multiplyer: 0.8,
            dropoff_powi: 4,
            dropoff_multiplyer: 1.5,
            final_val_addition: 0.25,
        }
    }
}
