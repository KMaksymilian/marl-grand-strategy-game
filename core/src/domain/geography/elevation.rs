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
        if elevation_val < 0.2 {
            Elevation::Ocean
        } else if elevation_val < 0.4 {
            Elevation::Lowland
        } else if elevation_val < 0.6 {
            Elevation::Plains
        } else if elevation_val < 0.8 {
            Elevation::Highland
        } else {
            Elevation::Mountain
        }
    }
}
