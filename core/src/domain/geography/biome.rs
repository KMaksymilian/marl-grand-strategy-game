use crate::domain::geography::elevation::*;
use crate::domain::geography::moisture::*;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Biome {
    Ocean,
    Desert,
    Grassland,
    Forest,
    Rainforest,
    Hills,
    Taiga,
    Tundra,
    Snow,
}
impl Biome {
    pub fn determine(elevation: &Elevation, moisture: &Moisture) -> Biome {
        match (elevation, moisture) {
            (Elevation::Ocean, _) => Biome::Ocean,

            (Elevation::Lowland, Moisture::Dry) => Biome::Desert,
            (Elevation::Lowland, Moisture::Normal) => Biome::Grassland,
            (Elevation::Lowland, Moisture::Humid) => Biome::Forest,
            (Elevation::Lowland, Moisture::Wet) => Biome::Rainforest,

            (Elevation::Plains, Moisture::Dry) => Biome::Desert,
            (Elevation::Plains, Moisture::Normal) => Biome::Grassland,
            (Elevation::Plains, Moisture::Humid) => Biome::Forest,
            (Elevation::Plains, Moisture::Wet) => Biome::Forest,

            (Elevation::Highland, Moisture::Dry) => Biome::Hills,
            (Elevation::Highland, Moisture::Normal) => Biome::Hills,
            (Elevation::Highland, Moisture::Humid) => Biome::Taiga,
            (Elevation::Highland, Moisture::Wet) => Biome::Taiga,

            (Elevation::Mountain, Moisture::Dry) => Biome::Tundra,
            (Elevation::Mountain, Moisture::Normal) => Biome::Tundra,
            (Elevation::Mountain, Moisture::Humid) => Biome::Snow,
            (Elevation::Mountain, Moisture::Wet) => Biome::Snow,
        }
    }
}
