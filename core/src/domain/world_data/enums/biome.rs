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
    let biomes_1: Vec<Biome> = get_biomes_from_elevation(elevation);
    let biomes_2: Vec<Biome> = get_biomes_from_moisture(moisture);
    if let Some(biome) = biomes_1.iter().find(|&&biome| biomes_2.contains(&biome)) {
        *biome
    } else {
        Biome::Ocean
    }
}

fn get_biomes_from_elevation(elevation: &Elevation) -> Vec<Biome> {
    match elevation {
        Elevation::LvL0 => vec![Biome::Ocean],
        Elevation::LvL1 => vec![
            Biome::SubtropicalDesert,
            Biome::Grassland,
            Biome::TropicalSeasonalForest,
            Biome::TropicalRainForest,
        ],
        Elevation::LvL2 => vec![
            Biome::TemperateDesert,
            Biome::Grassland,
            Biome::TemperateDeciduousForest,
            Biome::TemperateRainForest,
        ],
        Elevation::LvL3 => vec![Biome::TemperateDesert, Biome::Shrubland, Biome::Taiga],
        Elevation::LvL4 => vec![Biome::Scorched, Biome::Bare, Biome::Tundra, Biome::Snow],
    }
}

fn get_biomes_from_moisture(moisture: &Moisture) -> Vec<Biome> {
    match moisture {
        Moisture::LvL1 => vec![
            Biome::SubtropicalDesert,
            Biome::TemperateDesert,
            Biome::Scorched,
        ],
        Moisture::LvL2 => vec![Biome::Grassland, Biome::TemperateDesert, Biome::Bare],
        Moisture::LvL3 => vec![
            Biome::TropicalSeasonalForest,
            Biome::Grassland,
            Biome::Shrubland,
            Biome::Tundra,
        ],
        Moisture::LvL4 => vec![
            Biome::TropicalSeasonalForest,
            Biome::TemperateDeciduousForest,
            Biome::Shrubland,
            Biome::Snow,
        ],
        Moisture::LvL5 => vec![
            Biome::TropicalRainForest,
            Biome::TemperateDeciduousForest,
            Biome::Taiga,
            Biome::Snow,
        ],
        Moisture::LvL6 => vec![
            Biome::TropicalRainForest,
            Biome::TemperateRainForest,
            Biome::Taiga,
            Biome::Snow,
        ],
    }
}
