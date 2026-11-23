//! World builder state, initialization and generation pipeline.
mod provinces;
mod terrain;

use super::config::{
    ElevationTuningFineConfig, NoiseConfig, SeedConfig, VoronoiConfig, WarpConfig, WorldDimensions,
};
use super::deposits::DepositGenerationConfig;
use super::noise_manager::NoiseManager;
use crate::domain::economy::ResourceCatalog;
use crate::domain::world::terrain::area::Area;
use crate::domain::world::{World, map::Map};
use std::sync::Arc;

pub struct WorldBuilder {
    pub map: Map,
    pub border_noise_manager: NoiseManager,
    pub elevation_noise_manager: NoiseManager,
    pub moisture_noise_manager: NoiseManager,
    pub warp_noise_manager: NoiseManager,
}

impl WorldBuilder {
    pub fn generate_default() -> World {
        Self::generate_default_with_resources(Arc::new(ResourceCatalog::standard().clone()))
    }

    pub fn generate_default_with_resources(resources: Arc<ResourceCatalog>) -> World {
        Self::generate_with_resources(
            WorldDimensions::default(),
            SeedConfig::default(),
            NoiseConfig::default(),
            WarpConfig::default(),
            ElevationTuningFineConfig::default(),
            VoronoiConfig::default(),
            resources,
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
        Self::generate_with_resources(
            world_dimensions,
            seed_config,
            noise_config,
            warp_config,
            elevation_tuning_fine_config,
            voronoi_config,
            Arc::new(ResourceCatalog::standard().clone()),
        )
    }

    fn generate_with_resources(
        world_dimensions: WorldDimensions,
        seed_config: SeedConfig,
        noise_config: NoiseConfig,
        warp_config: WarpConfig,
        elevation_tuning_fine_config: ElevationTuningFineConfig,
        voronoi_config: VoronoiConfig,
        resources: Arc<ResourceCatalog>,
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
        let seed = seed_config.points_seed_option.unwrap_or_else(rand::random);
        let deposits = DepositGenerationConfig::for_catalog(seed, &resources);
        let mut world = World::with_resource_catalog(builder.map, provinces, resources);
        world
            .generate_deposits(&deposits)
            .expect("generated terrain and standard ore rules must be valid");
        world
        World::with_resource_catalog(builder.map, provinces, resources)
    }
}
