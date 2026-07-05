#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moisture {
    Dry,
    Normal,
    Humid,
    Wet,
}
impl Moisture {
    pub fn determine(value: f32) -> Moisture {
        if value < 0.4 {
            Moisture::Dry
        } else if value < 0.5 {
            Moisture::Normal
        } else if value < 0.6 {
            Moisture::Humid
        } else {
            Moisture::Wet
        }
    }
}
