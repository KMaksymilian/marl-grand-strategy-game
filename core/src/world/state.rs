use std::collections::HashMap;
mod construction;
mod inspection;
pub use inspection::TileView;

use super::territory::{TerritoryManager, Tile};
use crate::settlement::Settlement;
use crate::trade::caravan::Caravan;
use crate::world::{Position, coordinates::Point, map::Map, province::Province};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldError {
    Project(crate::settlement::projects::ProjectError),
    Building(crate::settlement::BuildingCommandError),
    DuplicateSettlement(u32),
    MissingSettlement(u32),
    /// Importing existing structures requires spatial registration (a later stage).
    SettlementNotEmpty(u32),
    MissingProvince(usize),
    ProvinceOccupied {
        province_id: usize,
        settlement_id: u32,
    },
    InvalidMapPosition(Position),
    SettlementOutsideProvince {
        province_id: usize,
        settlement_id: u32,
        registered_settlement: Option<u32>,
    },
}

pub struct World {
    pub map: Map,
    pub provinces: Vec<Province>,
    pub caravans: Vec<Caravan>,
    territory: TerritoryManager,
    settlements: HashMap<u32, Settlement>,
    province_settlements: HashMap<usize, u32>,
}
impl World {
    pub fn new(map: Map, provinces: Vec<Province>) -> World {
        World {
            map,
            provinces,
            caravans: Vec::new(),
            territory: TerritoryManager::default(),
            settlements: HashMap::new(),
            province_settlements: HashMap::new(),
        }
    }

    /// Registers an empty settlement in exactly one existing province.
    /// Neither registry changes when validation fails.
    pub fn add_settlement(
        &mut self,
        province_id: usize,
        settlement: Settlement,
    ) -> Result<(), WorldError> {
        let id = settlement.id();
        if self.settlements.contains_key(&id) {
            return Err(WorldError::DuplicateSettlement(id));
        }
        if let Some(settlement_id) = self.province_settlement(province_id)? {
            return Err(WorldError::ProvinceOccupied {
                province_id,
                settlement_id,
            });
        }
        if settlement.buildings().next().is_some() || !settlement.active_projects().is_empty() {
            return Err(WorldError::SettlementNotEmpty(id));
        }
        self.settlements.insert(id, settlement);
        self.province_settlements.insert(province_id, id);
        Ok(())
    }

    pub fn settlement(&self, id: u32) -> Option<&Settlement> {
        self.settlements.get(&id)
    }

    pub fn settlements(&self) -> impl Iterator<Item = &Settlement> {
        self.settlements.values()
    }

    /// Shared access only: mutations will be coordinated with settlements.
    pub fn territory(&self) -> &TerritoryManager {
        &self.territory
    }

    /// Returns the settlement ID in a province, or None for an empty province.
    pub fn province_settlement(&self, province_id: usize) -> Result<Option<u32>, WorldError> {
        if !self
            .provinces
            .iter()
            .any(|province| province.id == province_id)
        {
            return Err(WorldError::MissingProvince(province_id));
        }
        Ok(self.province_settlements.get(&province_id).copied())
    }

    /// Resolves a map position to an existing province ID, not a vector offset.
    pub fn province_at(&self, position: Position) -> Result<usize, WorldError> {
        let id = self
            .map
            .province_at(position)
            .ok_or(WorldError::InvalidMapPosition(position))?;
        self.province_settlement(id)?;
        Ok(id)
    }

    pub fn settlement_at(&self, position: Position) -> Result<Option<&Settlement>, WorldError> {
        let province_id = self.province_at(position)?;
        Ok(self
            .province_settlements
            .get(&province_id)
            .and_then(|id| self.settlements.get(id)))
    }

    /// Unlike the standalone registry, this read rejects invalid map positions.
    pub fn tile_at(&self, position: Position) -> Result<Tile, WorldError> {
        self.province_at(position)?;
        Ok(self.territory.tile_at(position))
    }

    /// Checks that the position belongs to the settlement's province.
    pub fn validate_settlement_position(
        &self,
        settlement_id: u32,
        position: Position,
    ) -> Result<(), WorldError> {
        if !self.settlements.contains_key(&settlement_id) {
            return Err(WorldError::MissingSettlement(settlement_id));
        }
        let province_id = self.province_at(position)?;
        let registered_settlement = self.province_settlement(province_id)?;
        if registered_settlement != Some(settlement_id) {
            return Err(WorldError::SettlementOutsideProvince {
                province_id,
                settlement_id,
                registered_settlement,
            });
        }
        Ok(())
    }
    pub fn caravan_travel(&mut self) {
        for caravan in &mut self.caravans {
            caravan.travel();
        }
    }
    pub fn centroid(&self, id: usize) -> Point {
        self.provinces[id].centroid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buildings::definition::BuildingDefinition;
    use crate::ids::ProjectId;
    use crate::settlement::projects::ProjectCost;
    use crate::world::terrain::area::Area;
    use crate::world::territory::TerritoryManager;
    use crate::world::territory::TileOccupancy;
    use std::sync::Arc;

    fn world() -> World {
        // IDs deliberately differ from province vector indices.
        World::new(
            Map::new(
                3,
                2,
                vec![Area::new(0.5, 0.5); 6],
                vec![10, 20, 30, 10, 20, 30],
            ),
            vec![Province::new(30), Province::new(10), Province::new(20)],
        )
    }

    fn settlement(id: u32) -> Settlement {
        Settlement::new(id, 100, 0, Arc::default())
    }

    #[test]
    fn world_starts_with_empty_provinces_and_tiles() {
        let world = world();
        assert_eq!(world.settlements().count(), 0);
        assert_eq!(world.province_settlement(10), Ok(None));
        assert!(
            world
                .settlement_at(Position { x: 0, y: 0 })
                .unwrap()
                .is_none()
        );
        assert_eq!(
            world.tile_at(Position { x: 0, y: 0 }).unwrap().occupancy(),
            TileOccupancy::Empty
        );
    }

    #[test]
    fn settlements_are_registered_in_distinct_provinces() {
        let mut world = world();
        world.add_settlement(10, settlement(1)).unwrap();
        world.add_settlement(30, settlement(2)).unwrap();

        for (x, province, settlement_id) in [(0, 10, 1), (2, 30, 2)] {
            for y in 0..2 {
                let position = Position { x, y };
                assert_eq!(world.province_at(position), Ok(province));
                assert_eq!(
                    world.settlement_at(position).unwrap().unwrap().id(),
                    settlement_id
                );
                assert_eq!(
                    world.validate_settlement_position(settlement_id, position),
                    Ok(())
                );
            }
        }
        assert_eq!(world.province_settlement(20), Ok(None));
        assert!(
            world
                .settlement_at(Position { x: 1, y: 0 })
                .unwrap()
                .is_none()
        );
        assert_eq!(world.settlements().count(), 2);
        assert_eq!(world.province_settlements.len(), 2);
    }

    #[test]
    fn occupied_and_missing_provinces_reject_registration_without_changes() {
        let mut world = world();
        world.add_settlement(10, settlement(1)).unwrap();
        let before = world.province_settlements.clone();
        assert_eq!(
            world.add_settlement(10, settlement(2)),
            Err(WorldError::ProvinceOccupied {
                province_id: 10,
                settlement_id: 1
            })
        );
        assert_eq!(
            world.add_settlement(99, settlement(3)),
            Err(WorldError::MissingProvince(99))
        );
        assert_eq!(world.province_settlements, before);
        assert_eq!(world.settlements().count(), 1);
        assert!(world.settlement(2).is_none());
        assert!(world.settlement(3).is_none());
    }

    #[test]
    fn settlement_cannot_be_registered_in_a_second_province() {
        let mut world = world();
        world
            .add_settlement(10, Settlement::new(1, 100, 7, Arc::default()))
            .unwrap();
        for province_id in [10, 20] {
            assert_eq!(
                world.add_settlement(province_id, settlement(1)),
                Err(WorldError::DuplicateSettlement(1))
            );
        }
        assert_eq!(world.settlement(1).unwrap().population(), 7);
        assert_eq!(world.settlements().count(), 1);
        assert_eq!(world.province_settlement(10), Ok(Some(1)));
        assert_eq!(world.province_settlement(20), Ok(None));
        assert_eq!(world.province_settlements.len(), 1);
        assert!(world.settlement(99).is_none());
    }

    #[test]
    fn preexisting_projects_cannot_bypass_spatial_registration() {
        let mut territory = TerritoryManager::default();
        let mut world = world();
        let mut settlement = settlement(1);
        let definition = Arc::new(BuildingDefinition {
            id: "house".into(),
            name: "House".into(),
            tier: 1,
            max_workers: 0,
            behaviors: vec![],
            construction_cost: ProjectCost::default(),
        });
        settlement
            .queue_construction(
                ProjectId(1),
                Position { x: 0, y: 0 },
                definition,
                ProjectCost::default(),
                &world.map,
                &mut territory,
            )
            .unwrap();
        assert_eq!(
            world.add_settlement(10, settlement),
            Err(WorldError::SettlementNotEmpty(1))
        );
        assert_eq!(world.settlements().count(), 0);
        assert!(world.province_settlements.is_empty());
    }

    #[test]
    fn preexisting_buildings_cannot_bypass_spatial_registration() {
        let mut territory = TerritoryManager::default();
        use crate::buildings::Building;
        use crate::ids::BuildingId;
        use crate::world::pathfinding::path_finder::PathFinder;

        let mut world = world();
        let mut settlement = settlement(1);
        let definition = Arc::new(BuildingDefinition {
            id: "house".into(),
            name: "House".into(),
            tier: 1,
            max_workers: 0,
            behaviors: vec![],
            construction_cost: ProjectCost::default(),
        });
        let position = Position { x: 0, y: 0 };
        let building = Building::new(BuildingId(1), position, definition).unwrap();
        settlement
            .add_building(
                building,
                &world.map,
                &mut PathFinder::new(3, 2),
                &mut territory,
            )
            .unwrap();
        assert_eq!(
            world.add_settlement(10, settlement),
            Err(WorldError::SettlementNotEmpty(1))
        );
        assert!(world.settlement(1).is_none());
        assert_eq!(world.province_settlement(10), Ok(None));
        assert_eq!(
            world.tile_at(position).unwrap().occupancy(),
            TileOccupancy::Empty
        );
    }

    #[test]
    fn position_validation_rejects_empty_and_other_settlements_provinces() {
        let mut world = world();
        world.add_settlement(30, settlement(1)).unwrap();
        world.add_settlement(10, settlement(2)).unwrap();
        for (position, province_id, registered_settlement) in [
            (Position { x: 0, y: 0 }, 10, Some(2)),
            (Position { x: 1, y: 0 }, 20, None),
        ] {
            assert_eq!(
                world.validate_settlement_position(1, position),
                Err(WorldError::SettlementOutsideProvince {
                    province_id,
                    settlement_id: 1,
                    registered_settlement
                })
            );
        }
        assert_eq!(
            world.validate_settlement_position(99, Position { x: 0, y: 0 }),
            Err(WorldError::MissingSettlement(99))
        );
    }

    #[test]
    fn invalid_positions_are_not_reported_as_empty_tiles() {
        let world = world();
        for position in [
            Position { x: -1, y: 0 },
            Position { x: 0, y: -1 },
            Position { x: 3, y: 0 },
            Position { x: 0, y: 2 },
            Position {
                x: i32::MAX,
                y: i32::MAX,
            },
        ] {
            assert_eq!(
                world.tile_at(position),
                Err(WorldError::InvalidMapPosition(position))
            );
            assert_eq!(
                world.province_at(position),
                Err(WorldError::InvalidMapPosition(position))
            );
            assert!(
                matches!(world.settlement_at(position), Err(WorldError::InvalidMapPosition(p)) if p == position)
            );
        }
    }

    #[test]
    fn malformed_map_layers_and_unknown_provinces_return_errors() {
        let mut world = world();
        let position = Position { x: 2, y: 1 };
        world.map.territory[5] = 99;
        assert_eq!(
            world.tile_at(position),
            Err(WorldError::MissingProvince(99))
        );
        world.map.territory.pop();
        assert_eq!(
            world.tile_at(position),
            Err(WorldError::InvalidMapPosition(position))
        );
        world.map.terrain.clear();
        assert_eq!(
            world.tile_at(Position { x: 0, y: 0 }),
            Err(WorldError::InvalidMapPosition(Position { x: 0, y: 0 }))
        );
    }
}
