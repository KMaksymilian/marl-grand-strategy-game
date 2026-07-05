use crate::domain::geography::{biome::Biome, elevation::Elevation, moisture::Moisture};
pub struct Area {
    pub biome: Biome,
    pub elevation: Elevation,
    pub elevation_val: f64,
    pub moisture: Moisture,
    pub moisture_val: f64,
}
impl Area {
    pub fn new(
        elevation: Elevation,
        elevation_val: f64,
        moisture: Moisture,
        moisture_val: f64,
    ) -> Area {
        let biome: Biome = Biome::determine(&elevation, &moisture);
        Area {
            biome,
            elevation,
            elevation_val,
            moisture,
            moisture_val,
        }
    }
}
