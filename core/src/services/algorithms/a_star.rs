use crate::domain::world_data::map::Map;
use crate::services::algorithms::path_finder::{DIAGONAL, PathFinder, STRAIGHT};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub struct AStar;

impl AStar {
    pub fn a_star(
        map: &Map,
        path_finder: &mut PathFinder,
        start: (usize, usize),
        end: (usize, usize),
    ) -> Option<Vec<(usize, usize)>> {
        let end_idx = PathFinder::point_to_idx(end, map.width);
        if PathFinder::is_ocean(map, PathFinder::point_to_idx(start, map.width))
            || PathFinder::is_ocean(map, end_idx)
        {
            return None;
        }

        path_finder.begin(map, start);
        let mut open_set: BinaryHeap<Reverse<(u32, (usize, usize))>> = BinaryHeap::new();
        open_set.push(Reverse((Self::heuristic(start, end), start)));

        while let Some(Reverse((_f_score, current))) = open_set.pop() {
            if current == end {
                return Some(path_finder.path_to(end_idx));
            }

            let current_idx = PathFinder::point_to_idx(current, map.width);
            if path_finder.is_closed(current_idx) {
                continue;
            }
            path_finder.close(current_idx);

            for (neighbor, step) in PathFinder::land_neighbors(map, current) {
                let neighbor_idx = PathFinder::point_to_idx(neighbor, map.width);
                if path_finder.is_closed(neighbor_idx) {
                    continue;
                }

                let g_score = path_finder
                    .g_score(current_idx)
                    .saturating_add(PathFinder::move_cost(map, current_idx, neighbor_idx, step));

                if path_finder.try_relax(neighbor_idx, g_score, current_idx) {
                    let f_score = g_score.saturating_add(Self::heuristic(neighbor, end));
                    open_set.push(Reverse((f_score, neighbor)));
                }
            }
        }
        None
    }

    fn heuristic(a: (usize, usize), b: (usize, usize)) -> u32 {
        let dx = a.0.abs_diff(b.0) as u32;
        let dy = a.1.abs_diff(b.1) as u32;
        let (min, max) = if dx < dy { (dx, dy) } else { (dy, dx) };
        DIAGONAL * min + STRAIGHT * (max - min)
    }
}
