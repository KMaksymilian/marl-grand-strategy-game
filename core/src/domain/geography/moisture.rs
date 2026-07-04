#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moisture {
    Dry,
    Normal,
    Humid,
    Wet,
}
impl Moisture {
    pub fn determine(value: f64) -> Moisture {
        if value < 0.3 {
            Moisture::Dry
        } else if value < 0.55 {
            Moisture::Normal
        } else if value < 0.8 {
            Moisture::Humid
        } else {
            Moisture::Wet
        }
    }
}
