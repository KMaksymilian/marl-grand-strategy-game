use crate::world::{Position, terrain::area::Area};
pub struct Map {
    pub width: usize,
    pub height: usize,
    pub terrain: Vec<Area>,
    pub territory: Vec<usize>,
}

impl Map {
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
