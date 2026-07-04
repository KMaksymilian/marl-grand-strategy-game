use crate::domain::world_data::{map::Map, point::Point, province::Province};

pub fn apply_voronoi(map: &mut Map, provinces: &mut [Province], points: &[Point]) {
    for province in provinces.iter_mut() {
        province.territory.clear();
    }

    for y in 0..map.height {
        for x in 0..map.width {
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
            map.territory[y][x] = province_idx;
            provinces[province_idx].territory.push(Point(x, y));
        }
    }
}

pub fn calculate_centroids(provinces: &[Province]) -> Vec<Point> {
    provinces.iter().map(|p| p.centroid()).collect()
}
