pub const PENALTY_WEIGHT: f32 = 10.0;
pub const STEP_COST: f32 = 1.0;

pub struct PathFinder {
    pub g_score: Vec<f32>,
    pub f_score: Vec<f32>,
    pub came_from: Vec<usize>,
    pub generation: Vec<u32>,
    pub current_generation: u32,
}

impl PathFinder {
    pub fn new(width: usize, height: usize) -> PathFinder {
        let size: usize = width * height;
        PathFinder {
            g_score: vec![f32::INFINITY; size],
            f_score: vec![f32::INFINITY; size],
            came_from: vec![usize::MAX; size],
            generation: vec![0; size],
            current_generation: 0,
        }
    }
    #[inline]
    pub fn ensure_current(&mut self, idx: usize) {
        if self.generation[idx] != self.current_generation {
            self.g_score[idx] = f32::INFINITY;
            self.f_score[idx] = f32::INFINITY;
            self.came_from[idx] = usize::MAX;
            self.generation[idx] = self.current_generation;
        }
    }
}

pub fn point_to_idx(point: (usize, usize), width: usize) -> usize {
    point.1 * width + point.0
}
pub fn h_function(start: (usize, usize), end: (usize, usize)) -> f32 {
    (start.0.abs_diff(end.0) + start.1.abs_diff(end.1)) as f32
}
pub fn get_neighbors(current: (usize, usize), width: usize, height: usize) -> Vec<(usize, usize)> {
    let mut neighbors: Vec<(usize, usize)> = Vec::with_capacity(4);
    let (x, y) = current;

    if x > 0 {
        neighbors.push((x - 1, y));
    }
    if y > 0 {
        neighbors.push((x, y - 1));
    }
    if x < width - 1 {
        neighbors.push((x + 1, y));
    }
    if y < height - 1 {
        neighbors.push((x, y + 1));
    }

    neighbors
}
pub fn retrieve_path(came_from: &[usize], mut current: usize) -> Vec<usize> {
    let mut path: Vec<usize> = Vec::new();
    path.push(current);

    while came_from[current] != usize::MAX {
        current = came_from[current];
        path.push(current);
    }

    path.reverse();
    path
}
