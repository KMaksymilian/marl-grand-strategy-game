#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moisture {
    Dry,
    Normal,
    Humid,
    Wet,
}
impl Moisture {
    pub const DRY_LEVEL: f32 = 0.4;
    pub const NORMAL_LEVEL: f32 = 0.5;
    pub const HUMID_LEVEL: f32 = 0.6;

    pub fn determine(value: f32) -> Moisture {
        if value < Self::DRY_LEVEL {
            Moisture::Dry
        } else if value < Self::NORMAL_LEVEL {
            Moisture::Normal
        } else if value < Self::HUMID_LEVEL {
            Moisture::Humid
        } else {
            Moisture::Wet
        }
    }
}
