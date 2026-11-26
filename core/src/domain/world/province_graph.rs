//! Which provinces border each other, for army movement (gameplay
//! specification §11.1), retreats and troop visibility.
//!
//! Two provinces are neighbours when a land tile of one shares an edge with a
//! land tile of the other. Ocean tiles connect nothing, and corners do not
//! count. Armies move from province to province on this graph, never tile by
//! tile, so a route can only cross provinces it is allowed to enter.
use super::map::Map;
use super::pathfinding::path_finder::PathFinder;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProvinceGraph {
    neighbors: BTreeMap<usize, BTreeSet<usize>>,
}

impl ProvinceGraph {
    /// Reads the borders from the map's territory and terrain layers.
    pub fn from_map(map: &Map) -> Self {
        let mut graph = Self::default();
        let land = |index: usize| {
            index < map.terrain.len()
                && index < map.territory.len()
                && !PathFinder::is_ocean(map, index)
        };
        if map.width == 0 {
            return graph;
        }
        // Only tiles backed by both layers: the declared size may be larger
        // than the layers (malformed maps), and must not be iterated.
        let tiles = map
            .terrain
            .len()
            .min(map.territory.len())
            .min(map.width.saturating_mul(map.height));
        for index in 0..tiles {
            if !land(index) {
                continue;
            }
            // Right and lower neighbours: every edge is visited once.
            let right = (index % map.width + 1 < map.width).then_some(index + 1);
            let below = index.checked_add(map.width);
            for other in [right, below].into_iter().flatten() {
                if other < tiles && land(other) && map.territory[other] != map.territory[index] {
                    graph.connect(map.territory[index], map.territory[other]);
                }
            }
        }
        graph
    }

    fn connect(&mut self, a: usize, b: usize) {
        self.neighbors.entry(a).or_default().insert(b);
        self.neighbors.entry(b).or_default().insert(a);
    }

    /// Bordering provinces, ascending ID; none for an unknown province.
    pub fn neighbors(&self, province: usize) -> impl Iterator<Item = usize> + '_ {
        self.neighbors
            .get(&province)
            .into_iter()
            .flat_map(|neighbors| neighbors.iter().copied())
    }

    pub fn are_neighbors(&self, a: usize, b: usize) -> bool {
        self.neighbors
            .get(&a)
            .is_some_and(|neighbors| neighbors.contains(&b))
    }

    /// The shortest route in provinces from `from` to `to` through provinces
    /// for which `allowed` holds (the target included, the start not
    /// checked). Breadth-first in ascending ID, so equal routes are always
    /// resolved the same way. The route lists the provinces to enter, without
    /// the start: empty when `from == to`, None when no route exists.
    pub fn route(
        &self,
        from: usize,
        to: usize,
        allowed: impl Fn(usize) -> bool,
    ) -> Option<Vec<usize>> {
        self.route_to_nearest(from, allowed, |province| province == to)
    }

    /// The shortest route to the nearest province for which `goal` holds,
    /// through provinces for which `allowed` holds; ties go to the lower ID
    /// along the way. Empty when the start is a goal, None when no goal can
    /// be reached.
    pub fn route_to_nearest(
        &self,
        from: usize,
        allowed: impl Fn(usize) -> bool,
        goal: impl Fn(usize) -> bool,
    ) -> Option<Vec<usize>> {
        if goal(from) {
            return Some(Vec::new());
        }
        let mut previous = BTreeMap::from([(from, from)]);
        let mut queue = VecDeque::from([from]);
        while let Some(current) = queue.pop_front() {
            for next in self.neighbors(current) {
                if previous.contains_key(&next) || !allowed(next) {
                    continue;
                }
                previous.insert(next, current);
                if goal(next) {
                    let mut route = vec![next];
                    let mut step = current;
                    while step != from {
                        route.push(step);
                        step = previous[&step];
                    }
                    route.reverse();
                    return Some(route);
                }
                queue.push_back(next);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::world::terrain::area::Area;

    /// Rows of province IDs; `0` marks an ocean tile of province 0.
    fn map(rows: &[&[usize]]) -> Map {
        let width = rows[0].len();
        let territory: Vec<usize> = rows.iter().flat_map(|row| row.iter().copied()).collect();
        let terrain = territory
            .iter()
            .map(|&province| {
                if province == 0 {
                    Area::new(0.0, 0.5)
                } else {
                    Area::new(0.5, 0.5)
                }
            })
            .collect();
        Map::new(width, rows.len(), terrain, territory)
    }

    #[test]
    fn provinces_sharing_a_land_edge_are_neighbors() {
        let graph = ProvinceGraph::from_map(&map(&[&[1, 1, 2], &[3, 0, 2], &[3, 4, 4]]));
        assert_eq!(graph.neighbors(1).collect::<Vec<_>>(), vec![2, 3]);
        assert_eq!(graph.neighbors(2).collect::<Vec<_>>(), vec![1, 4]);
        assert_eq!(graph.neighbors(4).collect::<Vec<_>>(), vec![2, 3]);
        assert!(graph.are_neighbors(3, 4) && graph.are_neighbors(4, 3));
        // Only a corner and the ocean lie between 1 and 4.
        assert!(!graph.are_neighbors(1, 4));
        assert_eq!(graph.neighbors(0).count(), 0, "ocean connects nothing");
        assert_eq!(graph.neighbors(99).count(), 0);
    }

    #[test]
    fn a_declared_size_larger_than_the_layers_is_not_iterated() {
        // Such a map once made the graph loop over usize::MAX² tiles.
        let huge = Map::new(usize::MAX, usize::MAX, vec![], vec![]);
        assert_eq!(ProvinceGraph::from_map(&huge), ProvinceGraph::default());
        let short = Map::new(4, 4, vec![Area::new(0.5, 0.5); 2], vec![1, 2]);
        assert!(ProvinceGraph::from_map(&short).are_neighbors(1, 2));
        let empty = Map::new(0, 3, vec![], vec![]);
        assert_eq!(ProvinceGraph::from_map(&empty), ProvinceGraph::default());
    }

    #[test]
    fn the_nearest_goal_is_found_through_allowed_provinces() {
        // 1 - 2 - 3
        // |   |   |
        // 4 - 5 - 6
        let graph = ProvinceGraph::from_map(&map(&[&[1, 2, 3], &[4, 5, 6]]));
        let goals = |p: usize| p == 3 || p == 4;
        assert_eq!(graph.route_to_nearest(1, |_| true, goals), Some(vec![4]));
        assert_eq!(
            graph.route_to_nearest(1, |p| p != 4, goals),
            Some(vec![2, 3])
        );
        assert_eq!(
            graph.route_to_nearest(3, |_| false, goals),
            Some(vec![]),
            "already there"
        );
        assert_eq!(graph.route_to_nearest(1, |p| p == 2, goals), None);
    }

    #[test]
    fn routes_are_shortest_deterministic_and_avoid_forbidden_provinces() {
        // 1 - 2 - 3
        // |   |   |
        // 4 - 5 - 6
        let graph = ProvinceGraph::from_map(&map(&[&[1, 2, 3], &[4, 5, 6]]));
        assert_eq!(graph.route(1, 1, |_| false), Some(vec![]));
        assert_eq!(graph.route(1, 3, |_| true), Some(vec![2, 3]));
        // Two equal routes to 5: through 2 (lower ID) is chosen.
        assert_eq!(graph.route(1, 5, |_| true), Some(vec![2, 5]));
        assert_eq!(graph.route(1, 5, |p| p != 2), Some(vec![4, 5]));
        assert_eq!(graph.route(1, 3, |p| p != 2), Some(vec![4, 5, 6, 3]));
        assert_eq!(graph.route(1, 3, |p| p != 2 && p != 5), None);
        assert_eq!(
            graph.route(1, 3, |p| p != 3),
            None,
            "the target must be allowed"
        );
        assert_eq!(graph.route(1, 99, |_| true), None);
    }
}
