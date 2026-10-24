use crate::{
    world::terrain::elevation::Elevation,
    world::{coordinates::Point, map::Map},
};

pub const STRAIGHT: u32 = 10;
pub const DIAGONAL: u32 = 14;
const SLOPE_SCALE: f32 = 1000.0;
const UPHILL_FACTOR: f32 = 1.5;

const DIRS: [(isize, isize, u32); 8] = [
    (-1, 0, STRAIGHT),
    (1, 0, STRAIGHT),
    (0, -1, STRAIGHT),
    (0, 1, STRAIGHT),
    (-1, -1, DIAGONAL),
    (1, -1, DIAGONAL),
    (-1, 1, DIAGONAL),
    (1, 1, DIAGONAL),
];

/// Per-tile search state. `seen` / `closed` hold the generation in which the
/// data is valid, so nothing has to be cleared between searches.
#[derive(Clone, Copy, Default)]
struct Node {
    seen: u32,
    closed: u32,
    g: u32,
    parent: Option<usize>,
}

/// Reusable scratch buffer for searches on a map of fixed size.
pub struct PathFinder {
    nodes: Vec<Node>,
    generation: u32,
    width: usize,
    height: usize,
}

impl PathFinder {
    pub fn new(width: usize, height: usize) -> PathFinder {
        PathFinder {
            nodes: vec![Node::default(); width * height],
            generation: 0,
            width,
            height,
        }
    }

    /// Starts a new search from `start`.
    pub fn begin(&mut self, map: &Map, start: (usize, usize)) {
        debug_assert!(self.matches_map(map));
        if self.generation == u32::MAX {
            self.nodes.fill(Node::default());
            self.generation = 1;
        } else {
            self.generation += 1;
        }
        let idx = Self::point_to_idx(start, self.width);
        self.set(idx, 0, None);
    }

    pub(crate) fn matches_map(&self, map: &Map) -> bool {
        self.width == map.width
            && self.height == map.height
            && map.width.checked_mul(map.height) == Some(self.nodes.len())
    }

    /// Best known cost to reach `idx` in the current search (`u32::MAX` if unreached).
    pub fn g_score(&self, idx: usize) -> u32 {
        let node = &self.nodes[idx];
        if node.seen == self.generation {
            node.g
        } else {
            u32::MAX
        }
    }

    pub fn is_closed(&self, idx: usize) -> bool {
        self.nodes[idx].closed == self.generation
    }

    pub fn close(&mut self, idx: usize) {
        self.nodes[idx].closed = self.generation;
    }

    /// Records the route to `idx` only if it is cheaper than the known one.
    /// Returns true when the node was improved (and should be pushed to the heap).
    pub fn try_relax(&mut self, idx: usize, g: u32, parent: usize) -> bool {
        if g < self.g_score(idx) {
            self.set(idx, g, Some(parent));
            true
        } else {
            false
        }
    }

    pub fn zone_to_points(zone: Vec<(usize, usize)>) -> Vec<Point> {
        let mut points: Vec<Point> = Vec::new();
        for point in zone {
            points.push(Point(point.0, point.1));
        }
        points
    }

    /// Path from the search start to `end_idx`, as map points (start first).
    pub fn path_to(&self, end_idx: usize) -> Vec<Point> {
        let mut path = vec![Self::idx_to_point(end_idx, self.width)];
        let mut current = end_idx;
        while let Some(prev) = self.parent(current) {
            path.push(Self::idx_to_point(prev, self.width));
            current = prev;
        }
        path.reverse();
        Self::path(path)
    }

    fn path(path_to: Vec<(usize, usize)>) -> Vec<Point> {
        let mut path: Vec<Point> = Vec::new();
        for point in path_to {
            path.push(Point(point.0, point.1));
        }
        path
    }

    fn parent(&self, idx: usize) -> Option<usize> {
        let node = &self.nodes[idx];
        if node.seen == self.generation {
            node.parent
        } else {
            None
        }
    }

    fn set(&mut self, idx: usize, g: u32, parent: Option<usize>) {
        let node = &mut self.nodes[idx];
        node.seen = self.generation;
        node.g = g;
        node.parent = parent;
    }

    pub fn point_to_idx(point: (usize, usize), width: usize) -> usize {
        point.1 * width + point.0
    }

    pub fn idx_to_point(idx: usize, width: usize) -> (usize, usize) {
        (idx % width, idx / width)
    }

    /// In-bounds, non-ocean neighbors of `current` with their step length.
    pub fn land_neighbors(
        map: &Map,
        current: (usize, usize),
    ) -> impl Iterator<Item = ((usize, usize), u32)> + '_ {
        DIRS.iter().filter_map(move |&(dx, dy, step)| {
            let x = current.0.checked_add_signed(dx)?;
            let y = current.1.checked_add_signed(dy)?;
            if x >= map.width || y >= map.height {
                return None;
            }
            if Self::is_ocean(map, Self::point_to_idx((x, y), map.width)) {
                return None;
            }
            Some(((x, y), step))
        })
    }

    pub fn is_ocean(map: &Map, idx: usize) -> bool {
        map.terrain[idx].elevation_val < Elevation::OCEAN_LEVEL
    }

    pub fn move_cost(map: &Map, from: usize, to: usize, step: u32) -> u32 {
        let d: f32 = map.terrain[to].elevation_val - map.terrain[from].elevation_val;
        let mut ratio: f32 = d.abs() * SLOPE_SCALE * (STRAIGHT as f32 / step as f32);
        if d > 0.0 {
            ratio *= UPHILL_FACTOR;
        }
        (step as f32 * (1.0 + ratio * ratio)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::pathfinding::{a_star::AStar, dijkstra_flood::DijkstraFlood};
    use crate::world::terrain::area::Area;

    #[test]
    fn generation_wrap_clears_stale_nodes_for_both_search_algorithms() {
        let map = Map::new(3, 3, vec![Area::new(0.5, 0.5); 9], vec![0; 9]);
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
        assert_eq!(finder.generation, 1);
        finder.generation = u32::MAX;
        let zone = DijkstraFlood::dijkstra_flood(&map, &mut finder, (1, 1), STRAIGHT);
        assert_eq!(zone.len(), 5);
        assert!(zone.contains(&Point(1, 1)));
        assert_eq!(
            AStar::travel_cost(&map, &mut finder, (1, 1), (1, 1)),
            Some(0)
        );
    }
}
