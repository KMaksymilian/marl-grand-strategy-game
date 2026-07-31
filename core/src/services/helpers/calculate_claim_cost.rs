// use std::collections::HashMap;
// use crate::domain::economy::resource::ResourceType;
// use crate::domain::settlement::projects::ProjectCost;
// use crate::domain::world_data::map::Map;
// use crate::services::algorithms::a_star::a_star;
// use crate::services::algorithms::path_finder::{point_to_idx, PathFinder};
//
// pub fn calculate_claim_cost(
//     map: &Map,
//     path_finder: &mut PathFinder,
//     settlement_center: (usize, usize),
//     target_tile: (usize, usize),
//     current_territory_size: u32,
// ) -> Option<ProjectCost> {
//     // using A* to calculate spath cost
//     let path = a_star(map, path_finder, settlement_center, target_tile);
//
//     if path.is_empty() && settlement_center != target_tile {
//         return None;
//     }
//
//     // 2. Calculate path cost
//     let target_idx = point_to_idx(target_tile, map.width);
//     let logistic_cost = path_finder.g_score[target_idx];
//
//     // 3. Cost formula
//     let base_cost = 50.0;
//     let sprawl_penalty = current_territory_size as f32 * 5.0;
//     let distance_penalty = logistic_cost * 3.0;
//
//     let total_labour_f32 = base_cost + sprawl_penalty + distance_penalty;
//
//     let total_labour = total_labour_f32.max(1.0).round() as u32;
//
//     // 4. Take cost to ProjectCost
//     let mut needed_resources = HashMap::new();
//     needed_resources.insert(ResourceType::Labour, total_labour);
//
//     let turn_cost = (total_labour / 100).max(1);
//
//     Some(ProjectCost {
//         needed_resources,
//         turn_cost,
//     })
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::domain::economy::resource::ResourceType;
//     use crate::domain::world_data::map::Map;
//     use crate::domain::geography::area::Area;
//     use crate::services::algorithms::path_finder::{point_to_idx, PathFinder};
//
//     /// Helper function
//     fn setup_pathfinding(width: usize, height: usize) -> (Map, PathFinder) {
//         let size = width * height;
//         let terrain = vec![Area::default(); size];
//         let territory = vec![0; size];
//
//         let map = Map::new(width, height, terrain, territory);
//         let path_finder = PathFinder::new(width, height);
//
//         (map, path_finder)
//     }
//
//     #[test]
//     fn test_calculate_claim_cost_returns_none_when_unreachable() {
//         // Arrange
//         let (mut map, mut path_finder) = setup_pathfinding(10, 10);
//         let center = (0, 0);
//         let target = (9, 9);
//         let territory_size = 5;
//
//
//          let wall_idx = point_to_idx((1, 0), map.width);
//          map.terrain[wall_idx].elevation_val = 9999.0;
//
//         // Act
//         let result = calculate_claim_cost(&map, &mut path_finder, center, target, territory_size);
//
//
//         // Assert
//         assert!(result.is_none());
//     }
//
//     #[test]
//     fn test_calculate_claim_cost_works_for_center_tile_itself() {
//         // Arrange
//         let (map, mut path_finder) = setup_pathfinding(10, 10);
//         let center = (5, 5);
//         let target = (5, 5);
//         let territory_size = 1;
//
//         // Act
//         let result = calculate_claim_cost(&map, &mut path_finder, center, target, territory_size);
//
//         // Assert
//         //  50.0 (base) + (1 * 5.0)(development) + (0.0)(distance) = 55 Labour
//         assert!(result.is_some());
//         let cost = result.unwrap();
//
//         assert_eq!(cost.needed_resources.get(&ResourceType::Labour), Some(&55));
//         assert_eq!(cost.turn_cost, 1);
//     }
//
//     #[test]
//     fn test_calculate_claim_cost_applies_sprawl_and_distance_penalties() {
//         // Arrange
//         let (map, mut path_finder) = setup_pathfinding(10, 10);
//         let center = (0, 0);
//         let target = (0, 2);
//         let territory_size = 10;
//
//         // Act
//         let result = calculate_claim_cost(&map, &mut path_finder, center, target, territory_size);
//
//         // Assert
//         assert!(result.is_some());
//         let cost = result.unwrap();
//
//         let target_idx = point_to_idx(target, map.width);
//         let logistic_cost = path_finder.g_score[target_idx];
//
//         let expected_labour_f32 = 50.0 + (10.0 * 5.0) + (logistic_cost * 3.0);
//         let expected_labour = expected_labour_f32.max(1.0).round() as u32;
//
//         assert_eq!(cost.needed_resources.get(&ResourceType::Labour), Some(&expected_labour));
//
//         let expected_turns = (expected_labour / 100).max(1);
//         assert_eq!(cost.turn_cost, expected_turns);
//     }
//
//     #[test]
//     fn test_calculate_claim_cost_scales_turn_cost_for_expensive_claims() {
//         // Arrange
//         let (map, mut path_finder) = setup_pathfinding(10, 10);
//         let center = (0, 0);
//         let target = (0, 1);
//         let territory_size = 100;
//
//
//         // Act
//         let result = calculate_claim_cost(&map, &mut path_finder, center, target, territory_size);
//
//         // Assert
//         assert!(result.is_some());
//         let cost = result.unwrap();
//
//         let target_idx = point_to_idx(target, map.width);
//         let logistic_cost = path_finder.g_score[target_idx];
//
//         let expected_labour = (50.0 + 500.0 + (logistic_cost * 3.0)).round() as u32;
//
//         assert_eq!(cost.needed_resources.get(&ResourceType::Labour), Some(&expected_labour));
//
//         let expected_turns = expected_labour / 100;
//         assert_eq!(cost.turn_cost, expected_turns);
//         assert!(cost.turn_cost >= 5);
//     }
// }