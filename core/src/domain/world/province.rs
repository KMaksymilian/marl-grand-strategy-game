pub struct Province {
    pub center: (usize, usize),
    pub tiles_id: Vec<usize>,
}

impl Province {
    pub fn new(x: usize, y: usize) -> Province {
        let center: (usize, usize) = (x, y);
        let tiles_id: Vec<usize> = Vec::new();
        Province { center, tiles_id }
    }
}
