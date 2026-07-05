use super::voronoi_math::{apply_voronoi, calculate_centroids, get_noise_value};
use crate::domain::geography::area::Area;
use crate::domain::geography::elevation::Elevation;
use crate::domain::geography::moisture::Moisture;
use crate::domain::world_data::{map::Map, point::Point, province::Province, world::World};
use crate::services::world_generation::voronoi_math::get_island_elevation_noise_value;

use noise::Perlin;
use rand::RngExt;
use rand::prelude::ThreadRng;

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
    pub borders_seed_option: Option<u32>,
    pub elevation_seed_option: Option<u32>,
    pub moisture_seed_option: Option<u32>,
}

pub struct ProceduralWorldGenerator;

impl ProceduralWorldGenerator {
    pub fn generate(config: &WorldGenerationConfig) -> World {
        let mut map: Map = Self::build_map(config);
        let provinces: Vec<Province> = Self::build_provinces(&mut map, config);
        World::new(map, provinces)
    }

    fn build_map(config: &WorldGenerationConfig) -> Map {
        let elevation_perlin: Perlin = Self::generate_perlin(config.elevation_seed_option);
        let moisture_perlin: Perlin = Self::generate_perlin(config.moisture_seed_option);

        let mut terrain: Vec<Vec<Area>> = Vec::with_capacity(config.height);
        let mut territory: Vec<Vec<usize>> = Vec::with_capacity(config.height);

        for y in 0..config.height {
            let mut terrain_row: Vec<Area> = Vec::with_capacity(config.width);
            for x in 0..config.width {
                let elevation_val: f64 = get_island_elevation_noise_value(
                    &elevation_perlin,
                    config.elevation_scale,
                    config.min_max,
                    x,
                    y,
                    config.width,
                    config.height,
                );
                let moisture_val: f64 =
                    get_noise_value(&moisture_perlin, config.moisture_scale, config.min_max, x, y);

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
        let border_perlin: Perlin = Self::generate_perlin(config.borders_seed_option);

        let mut points: Vec<Point> =
            Self::generate_random_points(config.province_count, config.width, config.height);

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

        provinces
    }

    fn generate_random_points(count: usize, width: usize, height: usize) -> Vec<Point> {
        let mut points: Vec<Point> = Vec::with_capacity(count);
        let mut rng: ThreadRng = rand::rng();

        while points.len() < count {
            let pt: Point = Point(rng.random_range(0..width), rng.random_range(0..height));
            if !points.contains(&pt) {
                points.push(pt);
            }
        }

        points
    }

    fn generate_perlin(seed_option: Option<u32>) -> Perlin {
        match seed_option {
            Some(seed) => Perlin::new(seed),
            None => {
                let mut rng: ThreadRng = rand::rng();
                let seed: u32 = rng.random();
                Perlin::new(seed)
            }
        }
    }
}
