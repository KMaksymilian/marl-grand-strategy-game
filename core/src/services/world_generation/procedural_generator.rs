use super::voronoi_math::{apply_voronoi, calculate_centroids};
use crate::domain::geography::area::Area;
use crate::domain::geography::elevation::Elevation;
use crate::domain::geography::moisture::Moisture;
use crate::domain::world_data::{map::Map, point::Point, province::Province, world::World};

use noise::{NoiseFn, Perlin};
use rand::RngExt;
use rand::prelude::ThreadRng;

pub struct WorldGenerationConfig {
    pub width: usize,
    pub height: usize,
    pub province_count: usize,
    pub iteration_count: usize,
    pub scale: f64,
    pub min_max: (f64, f64),
    pub elevation_seed: Option<u32>,
    pub moisture_seed: Option<u32>,
}

pub struct ProceduralWorldGenerator;

impl ProceduralWorldGenerator {
    pub fn generate(config: &WorldGenerationConfig) -> World {
        let mut map: Map = Self::build_map(config);
        let provinces: Vec<Province> = Self::build_provinces(&mut map, config);
        World::new(map, provinces)
    }

    fn build_map(config: &WorldGenerationConfig) -> Map {
        let elevation_perlin: Perlin = config.elevation_seed.map(Perlin::new).unwrap_or_default();
        let moisture_perlin: Perlin = config.moisture_seed.map(Perlin::new).unwrap_or_default();

        let mut terrain: Vec<Vec<Area>> = Vec::with_capacity(config.height);
        let mut territory: Vec<Vec<usize>> = Vec::with_capacity(config.height);

        for y in 0..config.height {
            let mut terrain_row: Vec<Area> = Vec::with_capacity(config.width);
            for x in 0..config.width {
                let elev_val: f64 = Self::get_noise_value(
                    &elevation_perlin,
                    config.scale,
                    config.min_max.0,
                    config.min_max.1,
                    x,
                    y,
                );
                let moist_val: f64 = Self::get_noise_value(
                    &moisture_perlin,
                    config.scale,
                    config.min_max.0,
                    config.min_max.1,
                    x,
                    y,
                );

                let elevation: Elevation = Elevation::determine(elev_val);
                let moisture: Moisture = Moisture::determine(moist_val);

                terrain_row.push(Area::new(elevation, moisture));
            }
            terrain.push(terrain_row);
            territory.push(vec![0; config.width]);
        }

        Map::new(config.width, config.height, terrain, territory)
    }

    fn build_provinces(map: &mut Map, config: &WorldGenerationConfig) -> Vec<Province> {
        let mut points: Vec<Point> =
            Self::generate_random_points(config.province_count, config.width, config.height);

        let mut provinces: Vec<Province> = points
            .iter()
            .enumerate()
            .map(|(idx, _)| Province::new(idx))
            .collect();

        for _ in 0..config.iteration_count {
            apply_voronoi(map, &mut provinces, &points);
            points = calculate_centroids(&provinces);
        }

        provinces
    }

    fn get_noise_value(perlin: &Perlin, scale: f64, min: f64, max: f64, x: usize, y: usize) -> f64 {
        let point: [f64; 2] = [x as f64 / scale, y as f64 / scale];
        let val: f64 = (perlin.get(point) + 1.0) / 2.0;
        val.clamp(min, max)
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
}
