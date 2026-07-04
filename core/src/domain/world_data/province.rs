#[derive(Copy, Clone, PartialEq)]
pub struct Point(pub usize, pub usize);

pub struct Province {
    pub id: usize,
    pub center: Point,
    pub territory: Vec<Point>,
}

impl Province {
    pub fn new(id: usize, center: Point) -> Province {
        let territory: Vec<Point> = Vec::new();
        Province {
            id,
            center,
            territory,
        }
    }
    pub fn centroid(&self) -> Point {
        let mut sum_x: usize = 0;
        let mut sum_y: usize = 0;

        for point in &self.territory {
            sum_x += point.0;
            sum_y += point.1;
        }

        let n: usize = self.territory.len();

        Point(sum_x / n, sum_y / n)
    }
}
