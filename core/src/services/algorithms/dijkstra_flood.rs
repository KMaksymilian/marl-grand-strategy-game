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
    ) -> Vec<(usize, usize)> {
        if PathFinder::is_ocean(map, PathFinder::point_to_idx(start, map.width)) {
            return Vec::new();
        }

        let mut zone: Vec<(usize, usize)> = Vec::new();
        let mut open_set: BinaryHeap<Reverse<(u32, (usize, usize))>> = BinaryHeap::new();

        path_finder.begin(map, start);
        open_set.push(Reverse((0, start)));

        while let Some(Reverse((current_cost, current))) = open_set.pop() {
            let current_idx = PathFinder::point_to_idx(current, map.width);
            if path_finder.is_closed(current_idx) {
                continue;
            }
            path_finder.close(current_idx);
            zone.push(current);

            for (neighbor, step) in PathFinder::land_neighbors(map, current) {
                let neighbor_idx = PathFinder::point_to_idx(neighbor, map.width);
                if path_finder.is_closed(neighbor_idx) {
                    continue;
                }

                let g_score = current_cost.saturating_add(PathFinder::move_cost(
                    map,
                    current_idx,
                    neighbor_idx,
                    step,
                ));
                if g_score > budget {
                    continue;
                }

                if path_finder.try_relax(neighbor_idx, g_score, current_idx) {
                    open_set.push(Reverse((g_score, neighbor)));
                }
            }
        }

        zone
    }
}
