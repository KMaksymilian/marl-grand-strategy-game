use super::voronoi_math::{apply_voronoi, calculate_centroids, get_noise_value, get_warp_value};
use crate::domain::geography::area::Area;
use crate::domain::geography::elevation::Elevation;
use crate::domain::geography::moisture::Moisture;
use crate::domain::world_data::{map::Map, point::Point, province::Province, world::World};
use crate::services::world_generation::voronoi_math::{
    edge_alignment, get_island_elevation_noise_value,
};
use noise::{Fbm, MultiFractal, Perlin};
use rand::prelude::ThreadRng;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

pub struct WorldGenerationConfig {
    pub width: usize,
    pub height: usize,
    pub province_count: usize,
    pub iteration_count: usize,
    pub elevation_scale: f64,
    pub moisture_scale: f64,
    pub min_max: (f64, f64),
    pub warp_scale: f64,
    pub warp_intensity: f64,
    pub points_seed_option: Option<u64>,
    pub borders_seed_option: Option<u32>,
    pub elevation_seed_option: Option<u32>,
    pub moisture_seed_option: Option<u32>,
}

pub struct ElevationTuningFineConfig {
    pub distance_multiplyer: f64,
    pub dropoff_powi: i32,
    pub dropoff_multiplyer: f64,
    pub final_val_addition: f64,
}

pub struct ProceduralWorldGenerator;

impl ProceduralWorldGenerator {
    pub fn generate(
        config: &WorldGenerationConfig,
        fine_config: &ElevationTuningFineConfig,
    ) -> World {
        let mut map: Map = Self::build_map(config, fine_config);
        let provinces: Vec<Province> = Self::build_provinces(&mut map, config);
        World::new(map, provinces)
    }

    fn build_map(config: &WorldGenerationConfig, fine_config: &ElevationTuningFineConfig) -> Map {
        let elevation_perlin: Fbm<Perlin> = Self::generate_fbm(config.elevation_seed_option);
        let moisture_perlin: Fbm<Perlin> = Self::generate_fbm(config.moisture_seed_option);

        let warp_perlin: Fbm<Perlin> = Self::generate_fbm(config.borders_seed_option);

        let mut terrain: Vec<Vec<Area>> = Vec::with_capacity(config.height);
        let mut territory: Vec<Vec<usize>> = Vec::with_capacity(config.height);

        for y in 0..config.height {
            let mut terrain_row: Vec<Area> = Vec::with_capacity(config.width);
            for x in 0..config.width {
                let (warped_x, warped_y) =
                    get_warp_value(&warp_perlin, config.warp_scale, config.warp_intensity, x, y);

                let elevation_val: f64 = get_island_elevation_noise_value(
                    &elevation_perlin,
                    config.elevation_scale,
                    config.min_max,
                    warped_x,
                    warped_y,
                    config.width,
                    config.height,
                    fine_config,
                );

                let moisture_val: f64 = get_noise_value(
                    &moisture_perlin,
                    config.moisture_scale,
                    config.min_max,
                    warped_x,
                    warped_y,
                );

                let elevation: Elevation = Elevation::determine(elevation_val);
                let moisture: Moisture = Moisture::determine(moisture_val);

                terrain_row.push(Area::new(elevation, moisture));
            }
            terrain.push(terrain_row);
            territory.push(vec![0; config.width]);
        }

        Map::new(config.width, config.height, terrain, territory)
    }

    fn build_provinces(map: &mut Map, config: &WorldGenerationConfig) -> Vec<Province> {
        let border_perlin: Fbm<Perlin> = Self::generate_fbm(config.borders_seed_option);

        let mut points: Vec<Point> = Self::generate_random_points(
            config.province_count,
            config.width,
            config.height,
            config.points_seed_option,
        );

        let mut provinces: Vec<Province> = points
            .iter()
            .enumerate()
            .map(|(idx, _)| Province::new(idx))
            .collect();

        for _ in 0..config.iteration_count {
            apply_voronoi(
                &border_perlin,
                config.warp_scale,
                config.warp_intensity,
                map,
                &mut provinces,
                &points,
            );
            points = calculate_centroids(&provinces);
        }

        edge_alignment(map, &mut provinces);

        provinces
    }

    fn generate_random_points(
        count: usize,
        width: usize,
        height: usize,
        seed_option: Option<u64>,
    ) -> Vec<Point> {
        let mut points: Vec<Point> = Vec::with_capacity(count);

        let seed: u64 = match seed_option {
            Some(s) => s,
            None => {
                let mut default_rng: ThreadRng = rand::rng();
                default_rng.random()
            }
        };

        let mut rng: StdRng = StdRng::seed_from_u64(seed);

        while points.len() < count {
            let pt: Point = Point(rng.random_range(0..width), rng.random_range(0..height));
            if !points.contains(&pt) {
                points.push(pt);
            }
        }

        points
    }

    fn generate_fbm(seed_option: Option<u32>) -> Fbm<Perlin> {
        let seed: u32 = match seed_option {
            Some(s) => s,
            None => {
                let mut rng: ThreadRng = rand::rng();
                rng.random()
            }
        };
        Fbm::<Perlin>::new(seed).set_octaves(4)
    }
}
