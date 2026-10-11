use crate::domain::world_data::map::Map;
use crate::services::algorithms::path_finder::DIAGONAL;
use crate::services::algorithms::path_finder::PathFinder;
use crate::services::algorithms::path_finder::STRAIGHT;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub struct AStar;

impl AStar {
    pub fn a_star(
        map: &Map,
        path_finder: &mut PathFinder,
        start: (usize, usize),
        end: (usize, usize),
    ) -> Option<Vec<usize>> {
        let start_idx: usize = PathFinder::point_to_idx(start, map.width);
        let end_idx: usize = PathFinder::point_to_idx(end, map.width);

        if PathFinder::is_ocean(map, start_idx) || PathFinder::is_ocean(map, end_idx) {
            return None;
        }

        let mut open_set: BinaryHeap<Reverse<(u32, usize, usize)>> = BinaryHeap::new();

        path_finder.new_generation(start, map.width);
        let start_h = Self::heuristic(start, end);
        open_set.push(Reverse((start_h, start.0, start.1)));

        while let Some(Reverse((_current_f, cx, cy))) = open_set.pop() {
            let current = (cx, cy);
            if current == end {
                return Some(Self::reconstruct_path(path_finder, end, map.width));
            }

            let current_idx: usize = PathFinder::point_to_idx(current, map.width);

            if path_finder.is_visited(current_idx) {
                continue;
            }
            path_finder.set_visited(current_idx);

            for (neighbor, step) in PathFinder::neighbors8(current, map.width, map.height) {
                let neighbor_idx = PathFinder::point_to_idx(neighbor, map.width);

                if path_finder.is_visited(neighbor_idx) || PathFinder::is_ocean(map, neighbor_idx) {
                    continue;
                }

                let tentative_g_score = path_finder
                    .get_g_score(current_idx)
                    .saturating_add(PathFinder::move_cost(map, current_idx, neighbor_idx, step));

                if tentative_g_score < path_finder.get_g_score(neighbor_idx) {
                    path_finder.set_came_from(neighbor_idx, Some(current));
                    path_finder.set_g_score(neighbor_idx, tentative_g_score);

                    let h_score: u32 = Self::heuristic(neighbor, end);
                    let f_score: u32 = tentative_g_score.saturating_add(h_score);

                    open_set.push(Reverse((f_score, neighbor.0, neighbor.1)));
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

    fn reconstruct_path(
        path_finder: &PathFinder,
        mut current: (usize, usize),
        width: usize,
    ) -> Vec<usize> {
        let mut path: Vec<usize> = vec![PathFinder::point_to_idx(current, width)];
        while let Some(prev) = path_finder.get_came_from(PathFinder::point_to_idx(current, width)) {
            current = prev;
            path.push(PathFinder::point_to_idx(current, width));
        }
        path.reverse();
        path
    }
}
