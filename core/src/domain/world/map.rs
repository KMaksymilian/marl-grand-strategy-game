use crate::domain::economy::ResourceType;
use crate::domain::spatial::site::{
    PROVINCE_DEPOSITS, SiteFactKind, SiteValue, TILE_BIOME, TILE_COASTAL, TILE_DEPOSITS,
};
use crate::domain::world::{Position, terrain::area::Area};
use std::collections::{BTreeSet, HashMap};
use std::fmt;

#[derive(Debug)]
pub enum SiteFactsConfigError {
    Parse(toml::de::Error),
    InvalidFactName(String),
    UnknownFact(String),
    InvalidValue {
        fact: String,
        expected: SiteFactKind,
    },
    InvalidPosition(Position),
    DuplicateTile(Position),
}

impl fmt::Display for SiteFactsConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "invalid site facts TOML: {error}"),
            Self::InvalidFactName(fact) => {
                write!(formatter, "invalid or reserved tile fact {fact:?}")
            }
            Self::UnknownFact(fact) => write!(formatter, "undeclared tile fact {fact:?}"),
            Self::InvalidValue { fact, expected } => {
                write!(formatter, "tile fact {fact:?} must have type {expected:?}")
            }
            Self::InvalidPosition(position) => {
                write!(formatter, "tile {position:?} is outside the map")
            }
            Self::DuplicateTile(position) => {
                write!(formatter, "tile {position:?} is configured twice")
            }
        }
    }
}

impl std::error::Error for SiteFactsConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            _ => None,
        }
    }
}

pub struct Map {
    pub width: usize,
    pub height: usize,
    pub terrain: Vec<Area>,
    pub territory: Vec<usize>,
    pub(super) deposits: HashMap<Position, ResourceType>,
    pub(super) site_fact_types: HashMap<String, SiteFactKind>,
    pub(super) site_facts: HashMap<Position, HashMap<String, SiteValue>>,
}

impl Map {
    pub fn deposits(&self) -> impl Iterator<Item = (Position, &ResourceType)> {
        self.deposits
            .iter()
            .map(|(&position, resource)| (position, resource))
    }

    /// Coastal tiles are land sharing an edge with an in-map ocean tile.
    pub fn is_coastal(&self, position: Position) -> bool {
        use super::terrain::elevation::Elevation;
        if self.validate_building_position(position).is_err() {
            return false;
        }
        [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .filter_map(|(dx, dy)| {
                Some(Position {
                    x: position.x.checked_add(dx)?,
                    y: position.y.checked_add(dy)?,
                })
            })
            .filter_map(|neighbor| self.tile_index(neighbor))
            .any(|index| self.terrain[index].determine_elevation() == Elevation::Ocean)
    }

    pub fn deposit_at(&self, position: Position) -> Option<ResourceType> {
        self.deposits.get(&position).cloned()
    }

    /// Reads one live site fact. A settlement cannot use deposits outside its province.
    pub(super) fn site_fact(
        &self,
        position: Position,
        owned_province: Option<usize>,
        id: &str,
    ) -> Option<SiteValue> {
        let index = self.tile_index(position)?;
        match id {
            TILE_COASTAL => Some(SiteValue::Bool(self.is_coastal(position))),
            TILE_BIOME => Some(self.terrain[index].determine_biome().into()),
            TILE_DEPOSITS | PROVINCE_DEPOSITS => {
                let province = self.territory[index];
                if owned_province.is_some_and(|owned| owned != province)
                    || self.validate_building_position(position).is_err()
                {
                    return Some(SiteValue::Deposits(Vec::new()));
                }
                let deposits = if id == TILE_DEPOSITS {
                    self.deposit_at(position).into_iter().collect()
                } else {
                    self.deposits()
                        .filter(|(deposit, _)| {
                            self.province_at(*deposit) == Some(province)
                                && self.validate_building_position(*deposit).is_ok()
                        })
                        .map(|(_, resource)| resource.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect()
                };
                Some(SiteValue::Deposits(deposits))
            }
            _ => self.site_facts.get(&position)?.get(id).cloned(),
        }
    }

    /// Current building types require land; use the terrain model's ocean boundary.
    pub fn validate_building_position(
        &self,
        position: Position,
    ) -> Result<(), super::territory::TerritoryError> {
        use super::terrain::elevation::Elevation;
        use super::territory::TerritoryError;
        let index = self
            .tile_index(position)
            .ok_or(TerritoryError::InvalidMapPosition(position))?;
        if self.terrain[index].determine_elevation() == Elevation::Ocean {
            return Err(TerritoryError::OceanTile(position));
        }
        Ok(())
    }

    /// Returns an index only for coordinates backed by both map layers.
    pub fn tile_index(&self, position: Position) -> Option<usize> {
        let x = usize::try_from(position.x).ok()?;
        let y = usize::try_from(position.y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = y.checked_mul(self.width)?.checked_add(x)?;
        (index < self.terrain.len() && index < self.territory.len()).then_some(index)
    }

    pub fn province_at(&self, position: Position) -> Option<usize> {
        self.tile_index(position).map(|index| self.territory[index])
    }

    pub fn new(width: usize, height: usize, terrain: Vec<Area>, territory: Vec<usize>) -> Map {
        Map {
            width,
            height,
            terrain,
            territory,
            deposits: HashMap::new(),
            site_fact_types: HashMap::new(),
            site_facts: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_indices_use_row_major_coordinates() {
        let map = Map::new(3, 2, vec![Area::default(); 6], vec![10, 20, 30, 40, 50, 60]);
        assert_eq!(map.tile_index(Position { x: 2, y: 1 }), Some(5));
        assert_eq!(map.province_at(Position { x: 1, y: 1 }), Some(50));
    }

    #[test]
    fn invalid_dimensions_and_index_overflow_do_not_panic() {
        for map in [
            Map::new(0, 0, vec![], vec![]),
            Map::new(usize::MAX, usize::MAX, vec![], vec![]),
        ] {
            assert_eq!(map.tile_index(Position { x: 1, y: 2 }), None);
        }
    }
}
