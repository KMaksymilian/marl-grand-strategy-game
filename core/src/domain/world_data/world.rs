use crate::domain::world_data::{
    area::Area,
    enums::{
        elevation::{Elevation, get_elevation},
        moisture::{Moisture, get_moisture},
    },
};
use noise::{NoiseFn, Perlin};

pub struct World {
    pub dim_w: usize,
    pub dim_h: usize,
    pub terrain: Vec<Vec<Area>>,
    pub territory: Vec<Vec<usize>>,
}

impl World {
    pub fn new_random(
        elevation_seed: Option<u32>,
        moisture_seed: Option<u32>,
        min: f64,
        max: f64,
        dim_w: usize,
        dim_h: usize,
    ) -> World {
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

        for y in 0..dim_w {
            let mut terrain_row: Vec<Area> = Vec::new();
            for x in 0..dim_h {
                let elevation_value: f64 = get_noise_value(&elevation_perlin, min, max, x, y);
                let moisture_value: f64 = get_noise_value(&moisture_perlin, min, max, x, y);
                let elevation: Elevation = get_elevation(elevation_value);
                let moisture: Moisture = get_moisture(moisture_value);
                terrain_row.push(Area::new(elevation, moisture));
            }
            terrain.push(terrain_row);
            territory.push(vec![0; dim_h]);
        }

        World {
            dim_w,
            dim_h,
            terrain,
            territory,
        }
    }
}

fn get_noise_value(perlin: &Perlin, min: f64, max: f64, x: usize, y: usize) -> f64 {
    let point: [f64; 2] = [x as f64, y as f64];
    let val: f64 = (perlin.get(point) + 1.0) / 2.0;
    val.clamp(min, max)
}
