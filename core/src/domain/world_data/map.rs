use crate::domain::geography::area::Area;
pub struct Map {
    pub width: usize,
    pub height: usize,
    pub terrain: Vec<Area>,
    pub territory: Vec<usize>,
}
impl Map {
    pub fn new(width: usize, height: usize, terrain: Vec<Area>, territory: Vec<usize>) -> Map {
        Map {
            width,
            height,
            terrain,
            territory,
        }
    }
}
