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
        Self::travel_cost(map, path_finder, start, end)?;
        Some(Self::reconstruct_path(path_finder, end, map.width))
    }

    /// Minimum directed movement cost using the same search as the map path view.
    /// Avoids allocating a path when only its cost is needed.
    pub fn travel_cost(
        map: &Map,
        path_finder: &mut PathFinder,
        start: (usize, usize),
        end: (usize, usize),
    ) -> Option<u32> {
        let size = map.width.checked_mul(map.height)?;
        if start.0 >= map.width
            || start.1 >= map.height
            || end.0 >= map.width
            || end.1 >= map.height
            || map.width > i32::MAX as usize
            || map.height > i32::MAX as usize
            || map.terrain.len() != size
            || path_finder.g_score.len() != size
            || path_finder.generated.len() != size
            || path_finder.visited.len() != size
            || path_finder.came_from.len() != size
        {
            return None;
        }
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
                return Some(path_finder.get_g_score(end_idx));
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
        DIAGONAL
            .saturating_mul(min)
            .saturating_add(STRAIGHT.saturating_mul(max - min))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geography::area::Area;

    fn map(width: usize, height: usize) -> Map {
        Map::new(
            width,
            height,
            vec![Area::new(0.5, 0.5); width * height],
            vec![0; width * height],
        )
    }

    fn path_cost(map: &Map, path: &[usize]) -> u32 {
        path.windows(2)
            .map(|pair| {
                let from = PathFinder::idx_to_point(pair[0], map.width);
                let to = PathFinder::idx_to_point(pair[1], map.width);
                let step = if from.0 != to.0 && from.1 != to.1 {
                    DIAGONAL
                } else {
                    STRAIGHT
                };
                PathFinder::move_cost(map, pair[0], pair[1], step)
            })
            .sum()
    }

    #[test]
    fn travel_cost_matches_the_displayed_path_including_diagonals_and_detours() {
        let mut map = map(3, 3);
        let mut finder = PathFinder::new(3, 3);
        assert_eq!(
            AStar::travel_cost(&map, &mut finder, (0, 0), (2, 2)),
            Some(28)
        );
        assert_eq!(
            AStar::travel_cost(&map, &mut finder, (0, 1), (2, 1)),
            Some(20)
        );
        map.terrain[4].elevation_val = 0.0;
        let route = AStar::a_star(&map, &mut finder, (0, 1), (2, 1)).unwrap();
        assert!(!route.contains(&4));
        assert_eq!(path_cost(&map, &route), 28);
        assert_eq!(
            AStar::travel_cost(&map, &mut finder, (0, 1), (2, 1)),
            Some(path_cost(&map, &route))
        );
    }

    #[test]
    fn elevation_cost_is_directed_and_matches_path_reconstruction() {
        let mut map = map(2, 1);
        map.terrain[1].elevation_val = 0.501;
        let mut finder = PathFinder::new(2, 1);
        let uphill = AStar::travel_cost(&map, &mut finder, (0, 0), (1, 0)).unwrap();
        let downhill = AStar::travel_cost(&map, &mut finder, (1, 0), (0, 0)).unwrap();
        assert!(uphill > downhill && downhill > STRAIGHT);
        let route = AStar::a_star(&map, &mut finder, (0, 0), (1, 0)).unwrap();
        assert_eq!(uphill, path_cost(&map, &route));
    }

    #[test]
    fn disconnected_water_and_invalid_inputs_do_not_produce_routes() {
        let mut map = map(3, 3);
        let mut finder = PathFinder::new(3, 3);
        for y in 0..3 {
            map.terrain[y * 3 + 1].elevation_val = 0.0;
        }
        assert_eq!(AStar::travel_cost(&map, &mut finder, (0, 1), (2, 1)), None);
        assert_eq!(AStar::travel_cost(&map, &mut finder, (1, 1), (1, 1)), None);
        assert_eq!(AStar::travel_cost(&map, &mut finder, (3, 0), (0, 0)), None);
        assert_eq!(AStar::travel_cost(&map, &mut finder, (0, 0), (0, 3)), None);
        assert_eq!(
            AStar::travel_cost(&map, &mut PathFinder::new(1, 1), (0, 0), (0, 0)),
            None
        );
        map.terrain.pop();
        assert_eq!(AStar::travel_cost(&map, &mut finder, (0, 0), (0, 0)), None);
        let empty = Map::new(0, 0, vec![], vec![]);
        assert_eq!(
            AStar::travel_cost(&empty, &mut PathFinder::new(0, 0), (0, 0), (0, 0)),
            None
        );
    }

    #[test]
    fn reused_buffers_remain_valid_after_generation_wrap() {
        let map = map(3, 3);
        let mut finder = PathFinder::new(3, 3);
        assert_eq!(
            AStar::travel_cost(&map, &mut finder, (0, 0), (2, 2)),
            Some(28)
        );
        finder.generation = u32::MAX;
        assert_eq!(
            AStar::travel_cost(&map, &mut finder, (2, 2), (0, 0)),
            Some(28)
        );
        assert_eq!(
            AStar::travel_cost(&map, &mut finder, (1, 1), (1, 1)),
            Some(0)
        );
    }
}
