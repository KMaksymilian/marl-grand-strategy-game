use crate::domain::world_data::{map::Map, province::Province};
pub struct World {
    pub map: Map,
    pub provinces: Vec<Province>,
}
impl World {
    pub fn new(map: Map, provinces: Vec<Province>) -> World {
        World { map, provinces }
    }
}
