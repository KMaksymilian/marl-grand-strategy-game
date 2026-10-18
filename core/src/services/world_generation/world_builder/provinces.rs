//! Province seeds, warped Voronoi relaxation and coast alignment.
use super::WorldBuilder;
use crate::domain::geography::elevation::Elevation;
use crate::domain::world_data::{point::Point, province::Province};
use crate::services::world_generation::config::{
    SeedConfig, VoronoiConfig, WarpConfig, WorldDimensions,
};
use crate::services::world_generation::world_builder_functions::WorldBuilderFunctions;
use rayon::prelude::*;

impl WorldBuilder {
    pub(super) fn build_provinces(
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

    pub(super) fn edge_alignment(&mut self, provinces: &mut [Province]) {
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
}
