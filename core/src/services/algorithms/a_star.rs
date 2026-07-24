use crate::{
    domain::{geography::elevation::Elevation, world_data::map::Map},
    services::algorithms::path_finder::{
        PENALTY_WEIGHT, PathFinder, STEP_COST, get_neighbors, h_function, point_to_idx,
        retrieve_path,
    },
};
use ordered_float::OrderedFloat;
use priority_queue::PriorityQueue;
use std::cmp::Reverse;

pub fn a_star(
    map: &Map,
    path_finder: &mut PathFinder,
    start: (usize, usize),
    end: (usize, usize),
) -> Vec<usize> {
    let width: usize = map.width;
    let height: usize = map.height;

    path_finder.current_generation += 1;

    // Reset
    if path_finder.current_generation == 0 {
        path_finder.generation.fill(0);
        path_finder.current_generation = 1;
    }

    let mut open_set: PriorityQueue<(usize, usize), Reverse<OrderedFloat<f32>>> =
        PriorityQueue::new();

    let start_idx: usize = point_to_idx(start, width);

    path_finder.ensure_current(start_idx);
    path_finder.g_score[start_idx] = 0.0;
    path_finder.f_score[start_idx] = h_function(start, end);

    open_set.push(start, Reverse(OrderedFloat(path_finder.f_score[start_idx])));

    while let Some((current, _)) = open_set.pop() {
        if current == end {
            return retrieve_path(&path_finder.came_from, point_to_idx(current, width));
        }

        let current_idx: usize = point_to_idx(current, width);

        for neighbor in get_neighbors(current, width, height) {
            let neighbor_idx: usize = point_to_idx(neighbor, width);

            if map.terrain[neighbor_idx].elevation_val < Elevation::OCEAN_LEVEL {
                continue;
            }

            let delta_elevation: f32 =
                map.terrain[current_idx].elevation_val - map.terrain[neighbor_idx].elevation_val;
            let cost: f32 = STEP_COST + delta_elevation.abs() * PENALTY_WEIGHT;

            path_finder.ensure_current(neighbor_idx);

            let suggested_g_score: f32 = path_finder.g_score[current_idx] + cost;

            if suggested_g_score < path_finder.g_score[neighbor_idx] {
                path_finder.came_from[neighbor_idx] = current_idx;
                path_finder.g_score[neighbor_idx] = suggested_g_score;

                let new_f_score: f32 = suggested_g_score + h_function(neighbor, end);
                path_finder.f_score[neighbor_idx] = new_f_score;

                open_set.push(neighbor, Reverse(OrderedFloat(new_f_score)));
            }
        }
    }

    Vec::new()
}
