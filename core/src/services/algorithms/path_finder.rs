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
    pub fn neighbors(current: (usize, usize), width: usize, height: usize) -> Vec<(usize, usize)> {
        let mut neighbors: Vec<(usize, usize)> = Vec::with_capacity(4);
        if current.0 > 0 {
            neighbors.push((current.0 - 1, current.1));
        }
        if current.0 < width - 1 {
            neighbors.push((current.0 + 1, current.1));
        }
        if current.1 > 0 {
            neighbors.push((current.0, current.1 - 1));
        }
        if current.1 < height - 1 {
            neighbors.push((current.0, current.1 + 1));
        }
        neighbors
    }
}
