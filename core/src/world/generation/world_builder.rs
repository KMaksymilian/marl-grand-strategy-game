//! World builder state, initialization and generation pipeline.
mod provinces;
mod terrain;

use super::config::{
    ElevationTuningFineConfig, NoiseConfig, SeedConfig, VoronoiConfig, WarpConfig, WorldDimensions,
};
use super::noise_manager::NoiseManager;
use crate::world::terrain::area::Area;
use crate::world::{World, map::Map};

pub struct WorldBuilder {
    pub map: Map,
    pub border_noise_manager: NoiseManager,
    pub elevation_noise_manager: NoiseManager,
    pub moisture_noise_manager: NoiseManager,
    pub warp_noise_manager: NoiseManager,
}

impl WorldBuilder {
    pub fn generate_default() -> World {
        WorldBuilder::generate(
            WorldDimensions::default(),
            SeedConfig::default(),
            NoiseConfig::default(),
            WarpConfig::default(),
            ElevationTuningFineConfig::default(),
            VoronoiConfig::default(),
        )
    }

    pub fn new(world_dimensions: &WorldDimensions, seed_config: &SeedConfig) -> WorldBuilder {
        let area_count: usize = world_dimensions.width * world_dimensions.height;
        let terrain: Vec<Area> = vec![Area::default(); area_count];
        let territory: Vec<usize> = vec![0; area_count];
        let map: Map = Map::new(
            world_dimensions.width,
            world_dimensions.height,
            terrain,
            territory,
        );

        WorldBuilder {
            map,
            border_noise_manager: NoiseManager::new(seed_config.borders_seed_option),
            elevation_noise_manager: NoiseManager::new(seed_config.elevation_seed_option),
            moisture_noise_manager: NoiseManager::new(seed_config.moisture_seed_option),
            warp_noise_manager: NoiseManager::new(seed_config.borders_seed_option),
        }
    }

    pub fn generate(
        world_dimensions: WorldDimensions,
        seed_config: SeedConfig,
        noise_config: NoiseConfig,
        warp_config: WarpConfig,
        elevation_tuning_fine_config: ElevationTuningFineConfig,
        voronoi_config: VoronoiConfig,
    ) -> World {
        let mut builder: WorldBuilder = WorldBuilder::new(&world_dimensions, &seed_config);
        builder.build_elevation(
            &world_dimensions,
            &noise_config,
            &warp_config,
            &elevation_tuning_fine_config,
        );
        builder.build_moisture(&world_dimensions, &noise_config, &warp_config);
        let (warp_field, mut provinces, points) = builder.build_provinces(
            &world_dimensions,
            &voronoi_config,
            &warp_config,
            &seed_config,
        );
        builder.build_voronoi(warp_field, &mut provinces, points, &voronoi_config);
        builder.edge_alignment(&mut provinces);
        builder.ocean_set();
        World::new(builder.map, provinces)
    }
}
