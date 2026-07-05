use crate::{
    domain::{
        geography::elevation::Elevation,
        world_data::{map::Map, point::Point, province::Province},
    },
    services::world_generation::procedural_generator::ElevationTuningFineConfig,
};
use noise::{Fbm, NoiseFn, Perlin};
use rayon::prelude::*;

pub fn apply_voronoi(
    warp_field: &[(usize, usize)],
    map: &mut Map,
    provinces: &mut [Province],
    points: &[Point],
) {
    map.territory
        .par_chunks_mut(map.width)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..map.width {
                let (warped_x, warped_y) = warp_field[y * map.width + x];
                let province_idx = points
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, point)| {
                        let dx = warped_x.abs_diff(point.0);
                        let dy = warped_y.abs_diff(point.1);
                        dx * dx + dy * dy
                    })
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);
                row[x] = province_idx;
            }
        });
    for province in provinces.iter_mut() {
        province.territory.clear();
    }
    for y in 0..map.height {
        for x in 0..map.width {
            let province_idx: usize = map.territory[y * map.width + x];
            provinces[province_idx].territory.push(Point(x, y));
        }
    }
}

pub fn edge_alignment(map: &mut Map, provinces: &mut [Province]) {
    let ocean_level: f32 = Elevation::OCEAN_LEVEL;
    for province in provinces {
        let mut is_ocean: bool = false;
        for point in &province.territory {
            if map.terrain[point.1 * map.width + point.0].elevation_val < ocean_level {
                is_ocean = true;
                break;
            }
        }
        if is_ocean {
            for point in &province.territory {
                map.terrain[point.1 * map.width + point.0].elevation_val = 0.0;
            }
        }
    }
}

pub fn ocean_set(map: &mut Map) {
    let ocean_level: f32 = Elevation::OCEAN_LEVEL;
    for area in &mut map.terrain {
        if area.elevation_val < ocean_level {
            area.elevation_val = 0.0;
            area.moisture_val = 1.0;
        }
    }
}

pub fn get_warp_value(
    border_perlin: &Fbm<Perlin>,
    warp_scale: f64,
    warp_intensity: f64,
    (x, y): (usize, usize),
) -> (usize, usize) {
    let noise_x: f64 = border_perlin.get([x as f64 / warp_scale, y as f64 / warp_scale]);
    let noise_y: f64 = border_perlin.get([y as f64 / warp_scale, x as f64 / warp_scale]);
    let warped_x: usize = (x as f64 + noise_x * warp_intensity) as usize;
    let warped_y: usize = (y as f64 + noise_y * warp_intensity) as usize;
    (warped_x, warped_y)
}

pub fn get_noise_value(
    perlin: &Fbm<Perlin>,
    scale: f64,
    (min, max): (f64, f64),
    (x, y): (usize, usize),
) -> f32 {
    let point: [f64; 2] = [x as f64 / scale, y as f64 / scale];
    let val: f64 = (perlin.get(point) + 1.0) / 2.0;
    val.clamp(min, max) as f32
}

pub fn get_island_elevation_noise_value(
    perlin: &Fbm<Perlin>,
    scale: f64,
    (min, max): (f64, f64),
    (x, y): (usize, usize),
    width: usize,
    height: usize,
    config: &ElevationTuningFineConfig,
) -> f32 {
    let point: [f64; 2] = [x as f64 / scale, y as f64 / scale];
    let val: f64 = (perlin.get(point) + 1.0) / 2.0;

    let nx: f64 = (x as f64 / width as f64) * 2.0 - 1.0;
    let ny: f64 = (y as f64 / height as f64) * 2.0 - 1.0;
    let dist: f64 = (nx * nx + ny * ny).sqrt() * config.distance_multiplyer;

    let dropoff: f64 = dist.powi(config.dropoff_powi) * config.dropoff_multiplyer;
    let final_val: f64 = val - dropoff + config.final_val_addition;

    final_val.clamp(min, max) as f32
}

pub fn calculate_centroids(provinces: &[Province]) -> Vec<Point> {
    provinces.iter().map(|p| p.centroid()).collect()
}
