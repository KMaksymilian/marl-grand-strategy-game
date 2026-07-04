pub enum Moisture {
    LvL1, // 1 Dry
    LvL2, // 2
    LvL3, // 3
    LvL4, // 4
    LvL5, // 5
    LvL6, // 6 Wet
}

pub fn get_moisture(moisture_value: f64) -> Moisture {
    if moisture_value < 0.25 {
        Moisture::LvL1
    } else if moisture_value < 0.4 {
        Moisture::LvL2
    } else if moisture_value < 0.55 {
        Moisture::LvL3
    } else if moisture_value < 0.7 {
        Moisture::LvL4
    } else if moisture_value < 0.85 {
        Moisture::LvL5
    } else {
        Moisture::LvL6
    }
}
