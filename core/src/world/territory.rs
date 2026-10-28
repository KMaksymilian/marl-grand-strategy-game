//! Global occupancy registry. Map bounds and ownership are validated by callers.
use std::collections::HashMap;

use crate::ids::{BuildingId, ProjectId};
use crate::world::Position;

/// Project IDs are local to their settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectReference {
    pub settlement_id: u32,
    pub project_id: ProjectId,
}

/// Building IDs are local to their settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingReference {
    pub settlement_id: u32,
    pub building_id: BuildingId,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TileOccupancy {
    #[default]
    Empty,
    Construction(ProjectReference),
    Building(BuildingReference),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    occupancy: TileOccupancy,
}

impl Tile {
    pub fn occupancy(&self) -> TileOccupancy {
        self.occupancy
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerritoryError {
    Occupied {
        position: Position,
        occupancy: TileOccupancy,
    },
    ReservationMismatch {
        position: Position,
        expected: ProjectReference,
        actual: TileOccupancy,
    },
    SettlementMismatch {
        project: ProjectReference,
        building: BuildingReference,
    },
}

/// Sparse spatial registry: absent entries represent empty tiles, not map bounds.
/// This registry validates occupancy only; project uniqueness belongs to settlements.
#[derive(Debug, Default)]
pub struct TerritoryManager {
    tiles: HashMap<Position, Tile>,
}

impl TerritoryManager {
    /// Returns a read-only snapshot. This does not validate map coordinates.
    pub fn tile_at(&self, position: Position) -> Tile {
        self.tiles.get(&position).copied().unwrap_or_default()
    }

    pub fn reserve_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError> {
        let occupancy = self.tile_at(position).occupancy();
        if occupancy != TileOccupancy::Empty {
            return Err(TerritoryError::Occupied {
                position,
                occupancy,
            });
        }

        self.tiles.insert(
            position,
            Tile {
                occupancy: TileOccupancy::Construction(project),
            },
        );
        Ok(())
    }

    pub fn cancel_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError> {
        self.validate_reservation(position, project)?;
        // Tile currently contains occupancy only. Preserve other data here when added.
        self.tiles.remove(&position);
        Ok(())
    }

    pub fn finish_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
        building: BuildingReference,
    ) -> Result<(), TerritoryError> {
        self.validate_reservation(position, project)?;
        if project.settlement_id != building.settlement_id {
            return Err(TerritoryError::SettlementMismatch { project, building });
        }

        self.tiles.insert(
            position,
            Tile {
                occupancy: TileOccupancy::Building(building),
            },
        );
        Ok(())
    }

    fn validate_reservation(
        &self,
        position: Position,
        expected: ProjectReference,
    ) -> Result<(), TerritoryError> {
        let actual = self.tile_at(position).occupancy();
        if actual != TileOccupancy::Construction(expected) {
            return Err(TerritoryError::ReservationMismatch {
                position,
                expected,
                actual,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSITION: Position = Position { x: 2, y: 3 };
    const PROJECT: ProjectReference = ProjectReference {
        settlement_id: 1,
        project_id: ProjectId(7),
    };
    const BUILDING: BuildingReference = BuildingReference {
        settlement_id: 1,
        building_id: BuildingId(4),
    };

    #[test]
    fn construction_lifecycle_reserves_releases_and_finishes() {
        let mut manager = TerritoryManager::default();
        assert_eq!(manager.tile_at(POSITION).occupancy(), TileOccupancy::Empty);
        assert!(manager.tiles.is_empty());

        manager.reserve_construction(POSITION, PROJECT).unwrap();
        assert_eq!(
            manager.tile_at(POSITION).occupancy(),
            TileOccupancy::Construction(PROJECT)
        );
        manager.cancel_construction(POSITION, PROJECT).unwrap();
        assert_eq!(manager.tile_at(POSITION).occupancy(), TileOccupancy::Empty);

        manager.reserve_construction(POSITION, PROJECT).unwrap();
        manager
            .finish_construction(POSITION, PROJECT, BUILDING)
            .unwrap();
        assert_eq!(
            manager.tile_at(POSITION).occupancy(),
            TileOccupancy::Building(BUILDING)
        );
    }

    #[test]
    fn occupied_tiles_reject_construction_without_changing_registry() {
        let mut manager = TerritoryManager::default();
        manager.reserve_construction(POSITION, PROJECT).unwrap();
        for occupancy in [
            TileOccupancy::Construction(PROJECT),
            TileOccupancy::Building(BUILDING),
        ] {
            let before = manager.tiles.clone();
            assert_eq!(
                manager.reserve_construction(POSITION, PROJECT),
                Err(TerritoryError::Occupied {
                    position: POSITION,
                    occupancy
                })
            );
            assert_eq!(manager.tiles, before);
            if matches!(occupancy, TileOccupancy::Construction(_)) {
                manager
                    .finish_construction(POSITION, PROJECT, BUILDING)
                    .unwrap();
            }
        }
    }

    #[test]
    fn wrong_project_or_settlement_cannot_cancel_or_finish_a_reservation() {
        let mut manager = TerritoryManager::default();
        manager.reserve_construction(POSITION, PROJECT).unwrap();
        let before = manager.tiles.clone();
        for wrong in [
            ProjectReference {
                project_id: ProjectId(8),
                ..PROJECT
            },
            ProjectReference {
                settlement_id: 2,
                ..PROJECT
            },
        ] {
            let error = TerritoryError::ReservationMismatch {
                position: POSITION,
                expected: wrong,
                actual: TileOccupancy::Construction(PROJECT),
            };
            assert_eq!(manager.cancel_construction(POSITION, wrong), Err(error));
            assert_eq!(
                manager.finish_construction(POSITION, wrong, BUILDING),
                Err(error)
            );
            assert_eq!(manager.tiles, before);
        }
    }

    #[test]
    fn completion_cannot_transfer_a_building_to_another_settlement() {
        let mut manager = TerritoryManager::default();
        manager.reserve_construction(POSITION, PROJECT).unwrap();
        let building = BuildingReference {
            settlement_id: 2,
            ..BUILDING
        };
        let before = manager.tiles.clone();
        assert_eq!(
            manager.finish_construction(POSITION, PROJECT, building),
            Err(TerritoryError::SettlementMismatch {
                project: PROJECT,
                building
            })
        );
        assert_eq!(manager.tiles, before);
    }

    #[test]
    fn empty_and_completed_tiles_cannot_be_canceled_or_completed() {
        let mut manager = TerritoryManager::default();
        for actual in [TileOccupancy::Empty, TileOccupancy::Building(BUILDING)] {
            let before = manager.tiles.clone();
            let error = TerritoryError::ReservationMismatch {
                position: POSITION,
                expected: PROJECT,
                actual,
            };
            assert_eq!(manager.cancel_construction(POSITION, PROJECT), Err(error));
            assert_eq!(
                manager.finish_construction(POSITION, PROJECT, BUILDING),
                Err(error)
            );
            assert_eq!(manager.tiles, before);
            if actual == TileOccupancy::Empty {
                manager.reserve_construction(POSITION, PROJECT).unwrap();
                manager
                    .finish_construction(POSITION, PROJECT, BUILDING)
                    .unwrap();
            }
        }
    }

    #[test]
    fn changes_on_one_tile_preserve_other_tiles() {
        let mut manager = TerritoryManager::default();
        let other_position = Position { x: 3, y: 2 };
        let other_project = ProjectReference {
            project_id: ProjectId(8),
            ..PROJECT
        };
        manager
            .reserve_construction(other_position, other_project)
            .unwrap();
        let other_tile = manager.tile_at(other_position);

        manager.reserve_construction(POSITION, PROJECT).unwrap();
        manager.cancel_construction(POSITION, PROJECT).unwrap();
        manager.reserve_construction(POSITION, PROJECT).unwrap();
        manager
            .finish_construction(POSITION, PROJECT, BUILDING)
            .unwrap();

        assert_eq!(manager.tile_at(other_position), other_tile);
        assert_eq!(manager.tiles.len(), 2);
    }
}
