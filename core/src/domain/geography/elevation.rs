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
        if elevation_val < 0.20 {
            Elevation::Ocean
        } else if elevation_val < 0.40 {
            Elevation::Lowland
        } else if elevation_val < 0.60 {
            Elevation::Plains
        } else if elevation_val < 0.80 {
            Elevation::Highland
        } else {
            Elevation::Mountain
        }
    }
}
