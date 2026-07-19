use ordered_float::OrderedFloat;
use priority_queue::PriorityQueue;
use std::cmp::Reverse;

use crate::{
    domain::{geography::elevation::Elevation, world_data::map::Map},
    services::algorithms::path_finder::{
        PENALTY_WEIGHT, PathFinder, STEP_COST, get_neighbors, point_to_idx,
    },
};

pub fn dijkstra_flood(
    map: &Map,
    path_finder: &mut PathFinder,
    start: (usize, usize),
    budget: f32,
) -> Vec<usize> {
    let width: usize = map.width;
    let height: usize = map.height;

    path_finder.current_generation += 1;

    if path_finder.current_generation == 0 {
        path_finder.generation.fill(0);
        path_finder.current_generation = 1;
    }

    let mut open_set: PriorityQueue<(usize, usize), Reverse<OrderedFloat<f32>>> =
        PriorityQueue::new();
    let mut zone: Vec<usize> = Vec::new();

    let start_idx: usize = point_to_idx(start, width);

    path_finder.ensure_current(start_idx);
    path_finder.g_score[start_idx] = 0.0;

    open_set.push(start, Reverse(OrderedFloat(0.0)));

    while let Some((current, _)) = open_set.pop() {
        let current_idx: usize = point_to_idx(current, width);
        zone.push(current_idx);

        for neighbor in get_neighbors(current, width, height) {
            let neighbor_idx: usize = point_to_idx(neighbor, width);

            if map.terrain[neighbor_idx].elevation_val < Elevation::OCEAN_LEVEL {
                continue;
            }

            let delta_elevation: f32 =
                map.terrain[current_idx].elevation_val - map.terrain[neighbor_idx].elevation_val;
            let cost: f32 = STEP_COST + delta_elevation.abs() * PENALTY_WEIGHT;

            path_finder.ensure_current(neighbor_idx);

            let suggested_score: f32 = path_finder.g_score[current_idx] + cost;

            if budget < suggested_score {
                continue;
            }

            if suggested_score < path_finder.g_score[neighbor_idx] {
                path_finder.g_score[neighbor_idx] = suggested_score;
                open_set.push(neighbor, Reverse(OrderedFloat(suggested_score)));
            }
        }
    }

    zone
}
