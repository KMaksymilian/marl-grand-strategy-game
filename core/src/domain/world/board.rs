use crate::domain::world::tile::*;
use noise::NoiseFn;
use noise::Perlin;
pub struct Board {
    pub tiles: Vec<Vec<Tile>>,
    pub dim_w: usize,
    pub dim_h: usize,
}

impl Board {
    pub fn new(noise_seed: Option<u32>, dim_w: usize, dim_h: usize, min: f64, max: f64) -> Board {
        let noise_map: Perlin = match noise_seed {
            Some(seed) => Perlin::new(seed),
            None => Perlin::default(),
        };

        let mut tiles: Vec<Vec<Tile>> = Vec::new();

        for i in 0..dim_h {
            let mut tile_row: Vec<Tile> = Vec::new();
            for j in 0..dim_w {
                let elevation: f64 = get_noise_value(&noise_map, i, j, min, max);
                tile_row.push(Tile::new(elevation, 0));
            }
            tiles.push(tile_row);
        }

        Board {
            tiles,
            dim_w,
            dim_h,
        }
    }
}

fn get_noise_value(noise_map: &Perlin, i: usize, j: usize, min: f64, max: f64) -> f64 {
    let point: [f64; 2] = [i as f64, j as f64];
    let value: f64 = (noise_map.get(point) + 1.0) / 2.0;
    value.clamp(min, max)
}
