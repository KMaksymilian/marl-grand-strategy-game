pub struct Tile {
    pub elevation: f64,
    pub province: usize,
}

impl Tile {
    pub fn new(elevation: f64, province: usize) -> Tile {
        Tile {
            elevation,
            province,
        }
    }
}
