use crate::domain::world_data::{
    map::Map,
    province::{Point, Province},
};
use ::rand::prelude::ThreadRng;
use rand::RngExt;

pub struct World {
    pub map: Map,
    pub provinces: Vec<Province>,
}

impl World {
    pub fn new_random(
        province_count: usize,
        iteration_count: usize,
        elevation_seed: Option<u32>,
        moisture_seed: Option<u32>,
        scale: f64,
        min_max: (f64, f64),
        dim: (usize, usize),
    ) -> World {
        let mut map: Map = Map::new_random(
            elevation_seed,
            moisture_seed,
            scale,
            min_max.0,
            min_max.1,
            dim.0,
            dim.1,
        );

        let mut points: Vec<Point> = generate_random_provinces(province_count, dim.0, dim.1);

        let mut provinces: Vec<Province> = points
            .iter()
            .enumerate()
            .map(|(idx, point)| Province::new(idx, *point))
            .collect();

        for _ in 0..iteration_count {
            voronoi(&mut map, &mut provinces, points);
            points = centroid(&mut provinces);
        }

        World { map, provinces }
    }
}

fn voronoi(map: &mut Map, provinces: &mut [Province], points: Vec<Point>) {
    for province in provinces.iter_mut() {
        province.territory.clear();
    }

    for x in 0..map.dim_h {
        for y in 0..map.dim_w {
            let mut min_val: usize = usize::MAX;
            let mut min_idx: usize = 0;
            for (idx, point) in points.iter().enumerate() {
                let dx: usize = x.abs_diff(point.0);
                let dy: usize = y.abs_diff(point.1);
                let cur_val: usize = dx * dx + dy * dy;
                if cur_val < min_val {
                    min_val = cur_val;
                    min_idx = idx;
                }
            }
            let province_idx: usize = min_idx;
            map.territory[x][y] = province_idx;

            provinces[province_idx].center = points[province_idx];
            provinces[province_idx].territory.push(Point(x, y));
        }
    }
}

fn centroid(provinces: &mut [Province]) -> Vec<Point> {
    let mut new_points: Vec<Point> = vec![Point(0, 0); provinces.len()];
    for (province_idx, province) in provinces.iter().enumerate() {
        let centroid: Point = province.centroid();
        new_points[province_idx] = centroid;
    }
    new_points
}

fn generate_random_provinces(province_count: usize, dim_w: usize, dim_h: usize) -> Vec<Point> {
    let mut counter: usize = 0;
    let mut points: Vec<Point> = Vec::new();
    let mut rng: ThreadRng = rand::rng();

    while counter < province_count {
        let w: usize = rng.random_range(0..dim_w);
        let h: usize = rng.random_range(0..dim_h);
        let point: Point = Point(w, h);
        if points.contains(&point) {
            continue;
        } else {
            points.push(point);
            counter += 1;
        }
    }

    points
}
