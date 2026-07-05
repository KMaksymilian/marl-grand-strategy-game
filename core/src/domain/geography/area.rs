use crate::domain::geography::{biome::Biome, elevation::Elevation, moisture::Moisture};
pub struct Area {
    pub elevation_val: f32,
    pub moisture_val: f32,
}
impl Area {
    pub fn new(elevation_val: f32, moisture_val: f32) -> Area {
        Area {
            elevation_val,
            moisture_val,
        }
    }
    pub fn determine_biome(&self) -> Biome {
        Biome::determine(
            &Self::determine_elevation(&self),
            &Self::determine_moisture(&self),
        )
    }
    pub fn determine_elevation(&self) -> Elevation {
        Elevation::determine(self.elevation_val)
    }
    pub fn determine_moisture(&self) -> Moisture {
        Moisture::determine(self.moisture_val)
    }
}
