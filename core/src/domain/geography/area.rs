use crate::domain::geography::{biome::Biome, elevation::Elevation, moisture::Moisture};
#[derive(Copy, Clone)]
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
        Biome::determine(&self.determine_elevation(), &self.determine_moisture())
    }
    pub fn determine_elevation(&self) -> Elevation {
        Elevation::determine(self.elevation_val)
    }
    pub fn determine_moisture(&self) -> Moisture {
        Moisture::determine(self.moisture_val)
    }
}
impl Default for Area {
    fn default() -> Area {
        Area {
            elevation_val: 0.0,
            moisture_val: 0.0,
        }
    }
}
