#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    Ocean,
    Lowland,
    Plains,
    Highland,
    Mountain,
}
impl Elevation {
    pub const OCEAN_LEVEL: f32 = 0.2;
    pub const LOWLAND_LEVEL: f32 = 0.4;
    pub const PLAINS_LEVEL: f32 = 0.6;
    pub const HIGHLAND_LEVEL: f32 = 0.8;

    pub fn determine(elevation_val: f32) -> Elevation {
        if elevation_val < Self::OCEAN_LEVEL {
            Elevation::Ocean
        } else if elevation_val < Self::LOWLAND_LEVEL {
            Elevation::Lowland
        } else if elevation_val < Self::PLAINS_LEVEL {
            Elevation::Plains
        } else if elevation_val < Self::HIGHLAND_LEVEL {
            Elevation::Highland
        } else {
            Elevation::Mountain
        }
    }
}
