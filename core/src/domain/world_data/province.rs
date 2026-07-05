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
        if n == 0 {
            return Point(0, 0);
        }

        let (sum_x, sum_y) = self.territory.iter().fold((0, 0), |(acc_x, acc_y), point| {
            (acc_x + point.0, acc_y + point.1)
        });

        Point(sum_x / n, sum_y / n)
    }
}
