use crate::domain::{geography::elevation::Elevation, world_data::map::Map};

pub const STRAIGHT: u32 = 10;
pub const DIAGONAL: u32 = 14;
const SLOPE_SCALE: f32 = 1000.0;
const UPHILL_FACTOR: f32 = 1.5;

const DIRS: [(i32, i32, u32); 8] = [
    (-1, 0, STRAIGHT),
    (1, 0, STRAIGHT),
    (0, -1, STRAIGHT),
    (0, 1, STRAIGHT),
    (-1, -1, DIAGONAL),
    (1, -1, DIAGONAL),
    (-1, 1, DIAGONAL),
    (1, 1, DIAGONAL),
];

pub struct PathFinder {
    pub g_score: Vec<u32>,
    pub visited: Vec<u32>,
    pub generated: Vec<u32>,
    pub came_from: Vec<Option<(usize, usize)>>,
    pub generation: u32,
}

impl PathFinder {
    pub fn new(width: usize, height: usize) -> PathFinder {
        let size: usize = width * height;
        PathFinder {
            g_score: vec![0; size],
            visited: vec![0; size],
            generated: vec![0; size],
            came_from: vec![None; size],
            generation: 0,
        }
    }

    pub fn new_generation(&mut self, start: (usize, usize), width: usize) {
        self.generation += 1;
        let start_idx: usize = PathFinder::point_to_idx(start, width);
        self.set_g_score(start_idx, 0);
        self.came_from[start_idx] = None; 
    }

    pub fn get_g_score(&self, idx: usize) -> u32 {
        if self.generated[idx] == self.generation {
            self.g_score[idx]
        } else {
            u32::MAX
        }
    }

    pub fn set_g_score(&mut self, idx: usize, val: u32) {
        self.generated[idx] = self.generation;
        self.g_score[idx] = val;
    }

    pub fn is_visited(&self, idx: usize) -> bool {
        self.visited[idx] == self.generation
    }

    pub fn set_visited(&mut self, idx: usize) {
        self.visited[idx] = self.generation;
    }

    pub fn get_came_from(&self, idx: usize) -> Option<(usize, usize)> {
        if self.generated[idx] == self.generation {
            self.came_from[idx]
        } else {
            None
        }
    }

    pub fn set_came_from(&mut self, idx: usize, val: Option<(usize, usize)>) {
        self.generated[idx] = self.generation;
        self.came_from[idx] = val;
    }

    pub fn point_to_idx(point: (usize, usize), width: usize) -> usize {
        point.1 * width + point.0
    }

    pub fn idx_to_point(idx: usize, width: usize) -> (usize, usize) {
        (idx % width, idx / width)
    }

    pub fn path_idx_to_point(idxes: Vec<usize>, width: usize) -> Vec<(usize, usize)> {
        let mut points: Vec<(usize, usize)> = Vec::with_capacity(idxes.len());
        for idx in idxes {
            points.push(Self::idx_to_point(idx, width));
        }
        points
    }

    pub fn neighbors8(
        current: (usize, usize),
        width: usize,
        height: usize,
    ) -> impl Iterator<Item = ((usize, usize), u32)> {
        DIRS.iter().filter_map(move |&(dx, dy, step)| {
            let x = current.0 as i32 + dx;
            let y = current.1 as i32 + dy;
            if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                None
            } else {
                Some(((x as usize, y as usize), step))
            }
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
