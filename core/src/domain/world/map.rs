use crate::domain::world::board::*;
use crate::domain::world::province::*;
pub struct Map {
    pub map: Board,
    pub provinces: Vec<Province>,
}

// impl World {
//     pub fn new(noise_seed: Option<u32>, dim_w: usize, dim_h: usize, min: f64, max: f64, province_count: usize) -> World {
//         let map: Map = Map::new(noise_seed, dim_w, dim_h, min, max);
//         let mut provinces: Vec<Province> = Vec::with_capacity(province_count);
//     }
// }

// fn generate_random_provinces(province_count: usize) -> Vec<(usize, usize)> {

// }
