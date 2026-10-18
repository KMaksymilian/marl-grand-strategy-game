//! Terrain channel generation and ocean normalization.
use super::WorldBuilder;
use crate::domain::geography::elevation::Elevation;
use crate::services::world_generation::config::{
    ElevationTuningFineConfig, NoiseConfig, WarpConfig, WorldDimensions,
};
use rayon::prelude::*;

impl WorldBuilder {
    pub(super) fn build_elevation(
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

    pub(super) fn build_moisture(
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

    pub(super) fn ocean_set(&mut self) {
        let ocean_level: f32 = Elevation::OCEAN_LEVEL;
        for area in &mut self.map.terrain {
            if area.elevation_val < ocean_level {
                area.elevation_val = 0.0;
                area.moisture_val = 1.0;
            }
        }
    }
}
