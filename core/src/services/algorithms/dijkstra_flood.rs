use crate::domain::world_data::map::Map;
use crate::services::algorithms::path_finder::PathFinder;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub struct DijkstraFlood;

impl DijkstraFlood {
    pub fn dijkstra_flood(
        map: &Map,
        path_finder: &mut PathFinder,
        start: (usize, usize),
        budget: u32,
    ) -> Vec<usize> {
        let start_idx: usize = PathFinder::point_to_idx(start, map.width);
        if PathFinder::is_ocean(map, start_idx) {
            return Vec::new();
        }

        let mut zone: Vec<usize> = Vec::new();
        let mut open_set: BinaryHeap<Reverse<(u32, usize, usize)>> = BinaryHeap::new();

        path_finder.new_generation(start, map.width);
        open_set.push(Reverse((0, start.0, start.1)));

        while let Some(Reverse((current_cost, cx, cy))) = open_set.pop() {
            let current = (cx, cy);
            let current_idx: usize = PathFinder::point_to_idx(current, map.width);

            if path_finder.is_visited(current_idx) {
                continue;
            }
            path_finder.set_visited(current_idx);
            zone.push(current_idx);

            for (neighbor, step) in PathFinder::neighbors8(current, map.width, map.height) {
                let neighbor_idx = PathFinder::point_to_idx(neighbor, map.width);
                if path_finder.is_visited(neighbor_idx) || PathFinder::is_ocean(map, neighbor_idx) {
                    continue;
                }

                let tentative_g_score = current_cost.saturating_add(PathFinder::move_cost(
                    map,
                    current_idx,
                    neighbor_idx,
                    step,
                ));
                if tentative_g_score > budget {
                    continue;
                }

                if tentative_g_score < path_finder.get_g_score(neighbor_idx) {
                    path_finder.set_g_score(neighbor_idx, tentative_g_score);
                    open_set.push(Reverse((tentative_g_score, neighbor.0, neighbor.1)));
                }
            }
        }

        zone
    }
}
