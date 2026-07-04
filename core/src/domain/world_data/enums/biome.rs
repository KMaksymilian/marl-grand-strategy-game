use crate::domain::world_data::enums::elevation::*;
use crate::domain::world_data::enums::moisture::*;
#[derive(Copy, Clone, PartialEq)]
pub enum Biome {
    Ocean,
    Snow,
    Tundra,
    Bare,
    Scorched,
    Taiga,
    Shrubland,
    TemperateDesert,
    TemperateRainForest,
    TemperateDeciduousForest,
    Grassland,
    TropicalRainForest,
    TropicalSeasonalForest,
    SubtropicalDesert,
}

pub fn get_biome(elevation: &Elevation, moisture: &Moisture) -> Biome {
    match (elevation, moisture) {
        (Elevation::LvL0, _) => Biome::Ocean,

        (Elevation::LvL1, Moisture::LvL1) => Biome::SubtropicalDesert,
        (Elevation::LvL1, Moisture::LvL2) => Biome::Grassland,
        (Elevation::LvL1, Moisture::LvL3) => Biome::TropicalSeasonalForest,
        (Elevation::LvL1, Moisture::LvL4) => Biome::TropicalSeasonalForest,
        (Elevation::LvL1, Moisture::LvL5) => Biome::TropicalRainForest,
        (Elevation::LvL1, Moisture::LvL6) => Biome::TropicalRainForest,

        (Elevation::LvL2, Moisture::LvL1) => Biome::TemperateDesert,
        (Elevation::LvL2, Moisture::LvL2) => Biome::Grassland,
        (Elevation::LvL2, Moisture::LvL3) => Biome::Grassland,
        (Elevation::LvL2, Moisture::LvL4) => Biome::TemperateDeciduousForest,
        (Elevation::LvL2, Moisture::LvL5) => Biome::TemperateDeciduousForest,
        (Elevation::LvL2, Moisture::LvL6) => Biome::TemperateRainForest,

        (Elevation::LvL3, Moisture::LvL1) => Biome::TemperateDesert,
        (Elevation::LvL3, Moisture::LvL2) => Biome::TemperateDesert,
        (Elevation::LvL3, Moisture::LvL3) => Biome::Shrubland,
        (Elevation::LvL3, Moisture::LvL4) => Biome::Shrubland,
        (Elevation::LvL3, Moisture::LvL5) => Biome::Taiga,
        (Elevation::LvL3, Moisture::LvL6) => Biome::Taiga,

        (Elevation::LvL4, Moisture::LvL1) => Biome::Scorched,
        (Elevation::LvL4, Moisture::LvL2) => Biome::Bare,
        (Elevation::LvL4, Moisture::LvL3) => Biome::Tundra,
        (Elevation::LvL4, Moisture::LvL4) => Biome::Snow,
        (Elevation::LvL4, Moisture::LvL5) => Biome::Snow,
        (Elevation::LvL4, Moisture::LvL6) => Biome::Snow,
    }
}
