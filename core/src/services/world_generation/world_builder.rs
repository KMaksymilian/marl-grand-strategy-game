use crate::{
    domain::{
        geography::{area::Area, elevation::Elevation},
        world_data::{map::Map, point::Point, province::Province, world::World},
    },
    services::world_generation::{
        config::{
            ElevationTuningFineConfig, NoiseConfig, SeedConfig, VoronoiConfig, WarpConfig,
            WorldDimensions,
        },
        noise_manager::NoiseManager,
        world_builder_functions::WorldBuilderFunctions,
    },
};
use rayon::prelude::*;

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

    fn build_elevation(
        &mut self,
        world_dimensions: &WorldDimensions,
        noise_config: &NoiseConfig,
        warp_config: &WarpConfig,
        fine_config: &ElevationTuningFineConfig,
    ) {
        self.map
            .terrain
            .par_chunks_mut(world_dimensions.width)
            .enumerate()
            .for_each(|(y, row)| {
                for (x, area) in row.iter_mut().enumerate() {
                    let (warped_x, warped_y) = self.warp_noise_manager.get_warp_value(
                        warp_config.warp_scale,
                        warp_config.warp_intensity,
                        (x, y),
                    );
                    let elevation_val: f32 = self
                        .elevation_noise_manager
                        .get_island_elevation_noise_value(
                            noise_config.elevation_scale,
                            noise_config.min_max,
                            (warped_x, warped_y),
                            world_dimensions.width,
                            world_dimensions.height,
                            fine_config,
                        );
                    area.elevation_val = elevation_val;
                }
            });
    }

    fn build_moisture(
        &mut self,
        world_dimensions: &WorldDimensions,
        noise_config: &NoiseConfig,
        warp_config: &WarpConfig,
    ) {
        self.map
            .terrain
            .par_chunks_mut(world_dimensions.width)
            .enumerate()
            .for_each(|(y, row)| {
                for (x, area) in row.iter_mut().enumerate() {
                    let (warped_x, warped_y) = self.warp_noise_manager.get_warp_value(
                        warp_config.warp_scale,
                        warp_config.warp_intensity,
                        (x, y),
                    );
                    let moisture_val: f32 = self.moisture_noise_manager.get_noise_value(
                        noise_config.moisture_scale,
                        noise_config.min_max,
                        (warped_x, warped_y),
                    );
                    area.moisture_val = moisture_val;
                }
            });
    }

    fn build_provinces(
        &mut self,
        world_dimensions: &WorldDimensions,
        voronoi_config: &VoronoiConfig,
        warp_config: &WarpConfig,
        seed_config: &SeedConfig,
    ) -> (Vec<(usize, usize)>, Vec<Province>, Vec<Point>) {
        let points: Vec<Point> = WorldBuilderFunctions::generate_random_points(
            voronoi_config.province_count,
            world_dimensions.width,
            world_dimensions.height,
            seed_config.points_seed_option,
        );

        let mut provinces: Vec<Province> = Vec::new();
        for idx in 0..points.len() {
            provinces.push(Province::new(idx));
        }

        let warp_field: Vec<(usize, usize)> = (0..world_dimensions.height)
            .into_par_iter()
            .flat_map(|y| {
                let mut row = Vec::with_capacity(world_dimensions.width);
                for x in 0..world_dimensions.width {
                    row.push(self.border_noise_manager.get_warp_value(
                        warp_config.warp_scale,
                        warp_config.warp_intensity,
                        (x, y),
                    ));
                }
                row
            })
            .collect();

        (warp_field, provinces, points)
    }

    pub fn build_voronoi(
        &mut self,
        warp_field: Vec<(usize, usize)>,
        provinces: &mut [Province],
        mut points: Vec<Point>,
        voronoi_config: &VoronoiConfig,
    ) {
        for _ in 0..voronoi_config.iteration_count {
            WorldBuilderFunctions::apply_voronoi(&warp_field, &mut self.map, provinces, &points);
            points = WorldBuilderFunctions::calculate_centroids(provinces);
        }
    }

    fn edge_alignment(&mut self, provinces: &mut [Province]) {
        let ocean_level: f32 = Elevation::OCEAN_LEVEL;
        for province in provinces {
            let mut is_ocean: bool = false;
            for point in &province.territory {
                if self.map.terrain[point.1 * self.map.width + point.0].elevation_val < ocean_level
                {
                    is_ocean = true;
                    break;
                }
            }
            if is_ocean {
                for point in &province.territory {
                    self.map.terrain[point.1 * self.map.width + point.0].elevation_val = 0.0;
                }
            }
        }
    }

    fn ocean_set(&mut self) {
        let ocean_level: f32 = Elevation::OCEAN_LEVEL;
        for area in &mut self.map.terrain {
            if area.elevation_val < ocean_level {
                area.elevation_val = 0.0;
                area.moisture_val = 1.0;
            }
        }
    }
}
