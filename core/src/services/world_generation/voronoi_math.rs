use crate::domain::world_data::{map::Map, point::Point, province::Province};
use noise::{NoiseFn, Perlin};

pub fn apply_voronoi(
    border_perlin: &Perlin,
    warp_scale: f64,
    warp_intensity: f64,
    map: &mut Map,
    provinces: &mut [Province],
    points: &[Point],
) {
    for province in provinces.iter_mut() {
        province.territory.clear();
    }

    for y in 0..map.height {
        for x in 0..map.width {
            let mut min_val: usize = usize::MAX;
            let mut min_idx: usize = 0;

            let (warped_x, warped_y) =
                get_warp_value(border_perlin, warp_scale, warp_intensity, x, y);

            for (idx, point) in points.iter().enumerate() {
                let dx: usize = warped_x.abs_diff(point.0);
                let dy: usize = warped_y.abs_diff(point.1);
                let cur_val: usize = dx * dx + dy * dy;
                if cur_val < min_val {
                    min_val = cur_val;
                    min_idx = idx;
                }
            }

            let province_idx: usize = min_idx;
            map.territory[y][x] = province_idx;
            provinces[province_idx].territory.push(Point(x, y));
        }
    }
}

fn get_warp_value(
    border_perlin: &Perlin,
    warp_scale: f64,
    warp_intensity: f64,
    x: usize,
    y: usize,
) -> (usize, usize) {
    let noise_x: f64 = border_perlin.get([x as f64 / warp_scale, y as f64 / warp_scale]);
    let noise_y: f64 = border_perlin.get([y as f64 / warp_scale, x as f64 / warp_scale]);
    let warped_x: usize = (x as f64 + noise_x * warp_intensity) as usize;
    let warped_y: usize = (y as f64 + noise_y * warp_intensity) as usize;
    (warped_x, warped_y)
}

pub fn get_noise_value(perlin: &Perlin, scale: f64, min: f64, max: f64, x: usize, y: usize) -> f64 {
    let point: [f64; 2] = [x as f64 / scale, y as f64 / scale];
    let val: f64 = (perlin.get(point) + 1.0) / 2.0;
    val.clamp(min, max)
}

pub fn calculate_centroids(provinces: &[Province]) -> Vec<Point> {
    provinces.iter().map(|p| p.centroid()).collect()
}
