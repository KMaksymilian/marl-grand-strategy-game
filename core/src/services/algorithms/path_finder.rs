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
    pub f_score: Vec<u32>,
    pub visited: Vec<bool>,
    pub came_from: Vec<Option<(usize, usize)>>,
}

impl PathFinder {
    pub fn new(width: usize, height: usize) -> PathFinder {
        let size: usize = width * height;
        PathFinder {
            g_score: vec![u32::MAX; size],
            f_score: vec![u32::MAX; size],
            visited: vec![false; size],
            came_from: vec![None; size],
        }
    }
    pub fn new_generation(&mut self, start: (usize, usize), width: usize) {
        self.g_score.fill(u32::MAX);
        self.f_score.fill(u32::MAX);
        self.visited.fill(false);
        self.came_from.fill(None);
        let start_idx: usize = PathFinder::point_to_idx(start, width);
        self.g_score[start_idx] = 0;
    }
    pub fn point_to_idx(point: (usize, usize), width: usize) -> usize {
        point.1 * width + point.0
    }
    pub fn idx_to_point(idx: usize, width: usize) -> (usize, usize) {
        (idx % width, idx / width)
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
