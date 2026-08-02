use crate::domain::geography::elevation::Elevation;
use crate::domain::world_data::map::Map;
use crate::services::algorithms::path_finder::PathFinder;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

const PENALTY: f32 = 100.0;
const MOVE_COST: u32 = 10;

pub struct DijkstraFlood;

impl DijkstraFlood {
    pub fn dijkstra_flood(
        map: &Map,
        path_finder: &mut PathFinder,
        start: (usize, usize),
        budget: u32,
    ) -> Vec<usize> {
        let mut zone: Vec<usize> = Vec::new();
        let mut open_set: BinaryHeap<Reverse<(u32, (usize, usize))>> = BinaryHeap::new();

        path_finder.new_generation(start, map.width);
        open_set.push(Reverse((0, start)));

        while let Some(Reverse((current_cost, current))) = open_set.pop() {
            let current_idx: usize = PathFinder::point_to_idx(current, map.width);
            if path_finder.visited[current_idx] {
                continue;
            }
            path_finder.visited[current_idx] = true;
            zone.push(current_idx);

            for neighbor in PathFinder::neighbors(current, map.width, map.height) {
                let neighbor_idx: usize = PathFinder::point_to_idx(neighbor, map.width);
                if path_finder.visited[neighbor_idx]
                    || map.terrain[neighbor_idx].elevation_val < Elevation::OCEAN_LEVEL
                {
                    continue;
                }

                let delta_elevation: f32 = map.terrain[current_idx].elevation_val
                    - map.terrain[neighbor_idx].elevation_val;
                let move_cost: u32 = MOVE_COST + (delta_elevation.abs() * PENALTY) as u32;
                let tentative_g_score: u32 = current_cost.saturating_add(move_cost);
                if tentative_g_score > budget {
                    continue;
                }

                if tentative_g_score < path_finder.g_score[neighbor_idx] {
                    path_finder.g_score[neighbor_idx] = tentative_g_score;
                    open_set.push(Reverse((tentative_g_score, neighbor)));
                }
            }
        }

        zone
    }
}
