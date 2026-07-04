use crate::domain::world_data::{
    area::Area,
    enums::{
        elevation::{Elevation, get_elevation},
        moisture::{Moisture, get_moisture},
    },
};
use noise::{NoiseFn, Perlin};

pub struct Map {
    pub dim_w: usize,
    pub dim_h: usize,
    pub terrain: Vec<Vec<Area>>,
    pub territory: Vec<Vec<usize>>,
}

impl Map {
    pub fn new_random(
        elevation_seed: Option<u32>,
        moisture_seed: Option<u32>,
        scale: f64,
        min: f64,
        max: f64,
        dim_w: usize,
        dim_h: usize,
    ) -> Map {
        let elevation_perlin: Perlin = match elevation_seed {
            Some(seed) => Perlin::new(seed),
            None => Perlin::default(),
        };
        let moisture_perlin: Perlin = match moisture_seed {
            Some(seed) => Perlin::new(seed),
            None => Perlin::default(),
        };

        let mut terrain: Vec<Vec<Area>> = Vec::new();
        let mut territory: Vec<Vec<usize>> = Vec::new();

        for x in 0..dim_h {
            let mut terrain_row: Vec<Area> = Vec::new();
            for y in 0..dim_w {
                let elevation_value: f64 =
                    get_noise_value(&elevation_perlin, scale, min, max, x, y);
                let moisture_value: f64 = get_noise_value(&moisture_perlin, scale, min, max, x, y);
                let elevation: Elevation = get_elevation(elevation_value);
                let moisture: Moisture = get_moisture(moisture_value);
                terrain_row.push(Area::new(elevation, moisture));
            }
            terrain.push(terrain_row);
            territory.push(vec![0; dim_w]);
        }

        Map {
            dim_w,
            dim_h,
            terrain,
            territory,
        }
    }
}

fn get_noise_value(perlin: &Perlin, scale: f64, min: f64, max: f64, x: usize, y: usize) -> f64 {
    let point: [f64; 2] = [x as f64 / scale, y as f64 / scale];
    let val: f64 = (perlin.get(point) + 1.0) / 2.0;
    val.clamp(min, max)
}
