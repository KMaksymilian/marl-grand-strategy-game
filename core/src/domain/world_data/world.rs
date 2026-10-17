use crate::domain::world_data::{caravan::Caravan, map::Map, point::Point, province::Province};
pub struct World {
    pub map: Map,
    pub provinces: Vec<Province>,
    pub caravans: Vec<Caravan>,
}
impl World {
    pub fn new(map: Map, provinces: Vec<Province>) -> World {
        World { map, provinces, caravans: Vec::new() }
    }
    pub fn centroid(&self, id: usize) -> Point {
        self.provinces[id].centroid()
    }
}
