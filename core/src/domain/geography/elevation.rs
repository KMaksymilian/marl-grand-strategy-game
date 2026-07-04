#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    Ocean,
    Lowland,
    Plains,
    Highland,
    Mountain,
}
impl Elevation {
    pub fn determine(elevation_val: f64) -> Elevation {
        if elevation_val < 0.25 {
            Elevation::Ocean
        } else if elevation_val < 0.45 {
            Elevation::Lowland
        } else if elevation_val < 0.65 {
            Elevation::Plains
        } else if elevation_val < 0.85 {
            Elevation::Highland
        } else {
            Elevation::Mountain
        }
    }
}
