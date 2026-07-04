use crate::domain::geography::area::Area;
pub struct Map {
    pub width: usize,
    pub height: usize,
    pub terrain: Vec<Vec<Area>>,
    pub territory: Vec<Vec<usize>>,
}
impl Map {
    pub fn new(
        width: usize,
        height: usize,
        terrain: Vec<Vec<Area>>,
        territory: Vec<Vec<usize>>,
    ) -> Map {
        Map {
            width,
            height,
            terrain,
            territory,
        }
    }
}
