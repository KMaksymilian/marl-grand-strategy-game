use crate::domain::world_data::point::Point;
pub struct Province {
    pub id: usize,
    pub territory: Vec<Point>,
}
impl Province {
    pub fn new(id: usize) -> Province {
        let territory: Vec<Point> = Vec::new();
        Province { id, territory }
    }

    pub fn centroid(&self) -> Point {
        let n: usize = self.territory.len();
        let mut sum_x: usize = 0;
        let mut sum_y: usize = 0;
        for point in &self.territory {
            sum_x += point.0;
            sum_y += point.1;
        }
        Point(sum_x / n, sum_y / n)
    }
}
