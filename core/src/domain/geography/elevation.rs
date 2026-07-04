#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    LvL0, // Ocean
    LvL1, // 1 Low
    LvL2, // 2
    LvL3, // 3
    LvL4, // 4 High
}
impl Elevation {
    pub fn determine(elevation_val: f64) -> Elevation {
        if elevation_val < 0.25 {
            Elevation::LvL0
        } else if elevation_val < 0.45 {
            Elevation::LvL1
        } else if elevation_val < 0.65 {
            Elevation::LvL2
        } else if elevation_val < 0.85 {
            Elevation::LvL3
        } else {
            Elevation::LvL4
        }
    }
}
