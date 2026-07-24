use crate::domain::world_data::{map::Map, point::Point, province::Province};
use rand::{
    RngExt, SeedableRng,
    rngs::{StdRng, ThreadRng},
};
use rayon::prelude::*;
pub struct WorldBuilderFunctions;
impl WorldBuilderFunctions {
    pub fn generate_random_points(
        count: usize,
        width: usize,
        height: usize,
        seed_option: Option<u64>,
    ) -> Vec<Point> {
        let mut points: Vec<Point> = Vec::with_capacity(count);

        let state: u64 = match seed_option {
            Some(s) => s,
            None => {
                let mut default_rng: ThreadRng = rand::rng();
                default_rng.random()
            }
        };

        let mut rng: StdRng = StdRng::seed_from_u64(state);

        while points.len() < count {
            let pt: Point = Point(rng.random_range(0..width), rng.random_range(0..height));
            if !points.contains(&pt) {
                points.push(pt);
            }
        }

        points
    }
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
                for (x, territory_province_idx) in row.iter_mut().enumerate() {
                    let (warped_x, warped_y) = warp_field[y * map.width + x];

                    let mut closest_province: usize = 0;
                    let mut closest_distance: usize = usize::MAX;

                    for (idx, point) in points.iter().enumerate() {
                        let dx: usize = warped_x.abs_diff(point.0);
                        let dy: usize = warped_y.abs_diff(point.1);
                        let distance: usize = dx * dx + dy * dy;

                        if distance < closest_distance {
                            closest_distance = distance;
                            closest_province = idx;
                        }
                    }
                    *territory_province_idx = closest_province;
                }
            });

        for province in provinces.iter_mut() {
            province.territory.clear();
        }

        for y in 0..map.height {
            for x in 0..map.width {
                let province_idx = map.territory[y * map.width + x];
                provinces[province_idx].territory.push(Point(x, y));
            }
        }
    }
    pub fn calculate_centroids(provinces: &[Province]) -> Vec<Point> {
        provinces.iter().map(|p| p.centroid()).collect()
    }
}
