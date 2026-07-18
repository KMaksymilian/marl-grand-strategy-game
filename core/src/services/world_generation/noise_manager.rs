use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use rand::{RngExt, rngs::ThreadRng};

use crate::services::world_generation::config::ElevationTuningFineConfig;
pub struct NoiseManager {
    pub noise: Fbm<Perlin>,
}
impl NoiseManager {
    pub fn new(seed_option: Option<u32>) -> NoiseManager {
        let seed: u32 = match seed_option {
            Some(s) => s,
            None => {
                let mut rng: ThreadRng = rand::rng();
                rng.random()
            }
        };
        let noise: Fbm<Perlin> = Fbm::<Perlin>::new(seed).set_octaves(4);
        NoiseManager { noise }
    }
    pub fn get_noise_value(
        &self,
        scale: f64,
        (min, max): (f64, f64),
        (x, y): (usize, usize),
    ) -> f32 {
        let point: [f64; 2] = [x as f64 / scale, y as f64 / scale];
        let val: f64 = (self.noise.get(point) + max) / 2.0;
        val.clamp(min, max) as f32
    }
    pub fn get_island_elevation_noise_value(
        &self,
        scale: f64,
        (min, max): (f64, f64),
        (x, y): (usize, usize),
        width: usize,
        height: usize,
        config: &ElevationTuningFineConfig,
    ) -> f32 {
        let point: [f64; 2] = [x as f64 / scale, y as f64 / scale];
        let val: f64 = (self.noise.get(point) + max) / 2.0;

        let nx: f64 = (x as f64 / width as f64) * 2.0 - 1.0;
        let ny: f64 = (y as f64 / height as f64) * 2.0 - 1.0;
        let dist: f64 = (nx * nx + ny * ny).sqrt() * config.distance_multiplyer;

        let dropoff: f64 = dist.powi(config.dropoff_powi) * config.dropoff_multiplyer;
        let mut final_val: f64 = val - dropoff + config.final_val_addition;

        if final_val > 0.85 {
            final_val = 0.85 + (final_val - 0.85) * 0.4;
        }

        final_val.clamp(min, max) as f32
    }
    pub fn get_warp_value(
        &self,
        warp_scale: f64,
        warp_intensity: f64,
        (x, y): (usize, usize),
    ) -> (usize, usize) {
        let noise_x: f64 = self
            .noise
            .get([x as f64 / warp_scale, y as f64 / warp_scale]);
        let noise_y: f64 = self
            .noise
            .get([y as f64 / warp_scale, x as f64 / warp_scale]);
        let warped_x: usize = (x as f64 + noise_x * warp_intensity) as usize;
        let warped_y: usize = (y as f64 + noise_y * warp_intensity) as usize;
        (warped_x, warped_y)
    }
}
