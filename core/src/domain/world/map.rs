use crate::domain::world::tile::*;
pub struct Map {
    pub tiles: Vec<Vec<Tile>>,
    pub dim_w: usize,
    pub dim_h: usize,
}
