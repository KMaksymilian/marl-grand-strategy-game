use crate::domain::buildings::core::Building;
use crate::domain::settlement::core::Settlement;
use crate::domain::world_data::{map::Map, province::Province};
pub struct World {
    pub map: Map,
    pub provinces: Vec<Province>,
    pub buildings: Vec<Building>,
    pub settlement: Vec<Settlement>
}
impl World {
    pub fn new(map: Map, provinces: Vec<Province>) -> World {
        World { map, provinces, buildings: vec![], settlement: vec![] }
    }
}
