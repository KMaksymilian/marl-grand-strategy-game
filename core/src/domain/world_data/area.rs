use crate::domain::world_data::enums::{
    biome::{Biome, get_biome},
    elevation::Elevation,
    moisture::Moisture,
};

pub struct Area {
    pub biome: Biome,
    pub elevation: Elevation,
    pub moisture: Moisture,
}

impl Area {
    pub fn new(elevation: Elevation, moisture: Moisture) -> Area {
        let biome: Biome = get_biome(&elevation, &moisture);
        Area {
            biome,
            elevation,
            moisture,
        }
    }
}
