use crate::domain::geography::{biome::Biome, elevation::Elevation, moisture::Moisture};
pub struct Area {
    pub biome: Biome,
    pub elevation: Elevation,
    pub moisture: Moisture,
}
impl Area {
    pub fn new(elevation: Elevation, moisture: Moisture) -> Area {
        let biome: Biome = Biome::determine(&elevation, &moisture);
        Area {
            biome,
            elevation,
            moisture,
        }
    }
}
