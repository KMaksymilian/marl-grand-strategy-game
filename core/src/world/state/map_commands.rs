//! Controlled edits to existing map tiles. Layer shape and province registry stay fixed.
use super::{World, WorldError};
use crate::spatial::{Point, Position};
use crate::world::terrain::area::Area;

impl World {
    /// Terrain changes are observed by project validation and productivity next turn.
    /// Flooding a reserved site does not silently delete its project or reservation.
    pub fn set_tile_terrain(
        &mut self,
        position: Position,
        terrain: Area,
    ) -> Result<(), WorldError> {
        let index = self
            .map
            .tile_index(position)
            .ok_or(WorldError::InvalidMapPosition(position))?;
        if !terrain.elevation_val.is_finite() || !terrain.moisture_val.is_finite() {
            return Err(WorldError::InvalidTerrain(position));
        }
        self.map.terrain[index] = terrain;
        Ok(())
    }

    /// Change membership to an existing province and synchronize its tile lists.
    /// Settlements and structural references retain their identities. Pending
    /// construction outside its settlement's province blocks the next turn until
    /// canceled or restored, using the existing preflight checks.
    pub fn set_tile_province(
        &mut self,
        position: Position,
        province_id: usize,
    ) -> Result<(), WorldError> {
        let index = self
            .map
            .tile_index(position)
            .ok_or(WorldError::InvalidMapPosition(position))?;
        self.province(province_id)?;
        let previous = self.map.territory[index];
        let point = Point(position.x as usize, position.y as usize);
        if previous != province_id {
            if let Some(province) = self
                .provinces
                .iter_mut()
                .find(|province| province.id == previous)
            {
                province.territory.retain(|tile| *tile != point);
            }
            self.map.territory[index] = province_id;
        }
        let province = self
            .provinces
            .iter_mut()
            .find(|province| province.id == province_id)
            .expect("validated province");
        if !province.territory.contains(&point) {
            province.territory.push(point);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{Map, Province};

    fn world() -> World {
        let mut first = Province::new(10);
        first.territory = vec![Point(0, 0), Point(1, 0)];
        World::new(
            Map::new(2, 1, vec![Area::new(0.5, 0.5); 2], vec![10, 10]),
            vec![first, Province::new(20)],
        )
    }

    #[test]
    fn membership_edits_keep_map_and_province_lists_in_sync() {
        let mut world = world();
        let position = Position { x: 1, y: 0 };
        for _ in 0..2 {
            world.set_tile_province(position, 20).unwrap();
            assert_eq!(world.province_at(position), Ok(20));
            assert_eq!(world.province(10).unwrap().territory, vec![Point(0, 0)]);
            assert_eq!(world.province(20).unwrap().territory, vec![Point(1, 0)]);
            assert_eq!(world.centroid(20), Ok(Point(1, 0)));
        }
        world.set_tile_province(position, 10).unwrap();
        assert!(world.province(20).unwrap().territory.is_empty());
        assert_eq!(world.province(10).unwrap().territory.len(), 2);
        assert_eq!(world.centroid(99), Err(WorldError::MissingProvince(99)));
    }

    #[test]
    fn invalid_edits_preserve_terrain_membership_and_layer_shape() {
        let mut world = world();
        let position = Position { x: 0, y: 0 };
        for terrain in [Area::new(f32::NAN, 0.5), Area::new(0.5, f32::INFINITY)] {
            assert_eq!(
                world.set_tile_terrain(position, terrain),
                Err(WorldError::InvalidTerrain(position))
            );
        }
        assert_eq!(
            world.set_tile_province(position, 99),
            Err(WorldError::MissingProvince(99))
        );
        let outside = Position { x: 2, y: 0 };
        assert_eq!(
            world.set_tile_terrain(outside, Area::default()),
            Err(WorldError::InvalidMapPosition(outside))
        );
        assert_eq!(
            world.set_tile_province(outside, 20),
            Err(WorldError::InvalidMapPosition(outside))
        );
        assert_eq!(world.map().terrain[0].elevation_val, 0.5);
        assert_eq!(world.map().terrain[0].moisture_val, 0.5);
        assert_eq!(world.map().territory, vec![10, 10]);
        assert_eq!(world.map().terrain.len(), 2);
        assert_eq!(
            world.province(10).unwrap().territory,
            vec![Point(0, 0), Point(1, 0)]
        );
        assert!(world.province(20).unwrap().territory.is_empty());
    }
}
