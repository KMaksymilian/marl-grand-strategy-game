//! Map-independent coordinates, occupancy values and construction contracts.
use crate::domain::ids::{BuildingId, ProjectId, SettlementId};
use serde::{Deserialize, Serialize};

/// Grid coordinates. Point uses nonnegative indices; Position retains signed placement coordinates.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Point(pub usize, pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

/// Project IDs are local to their settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectReference {
    pub settlement_id: SettlementId,
    pub project_id: ProjectId,
}

/// Building IDs are local to their settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingReference {
    pub settlement_id: SettlementId,
    pub building_id: BuildingId,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TileOccupancy {
    #[default]
    Empty,
    Construction(ProjectReference),
    Building(BuildingReference),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerritoryError {
    InvalidMapPosition(Position),
    OceanTile(Position),
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

/// Narrow port for construction reservations. Implementations own placement rules.
///
/// Use one persistent occupancy store across all calls and settlements. References
/// are qualified by settlement identity. Returning `Err` must leave occupancy
/// unchanged; successful operations must be visible to subsequent validation.
pub trait ConstructionSpace {
    /// Whether this settlement can work a deposit on the specified tile.
    fn can_extract(
        &self,
        _position: Position,
        _resource: crate::domain::economy::ResourceType,
    ) -> bool {
        false
    }

    /// Validate placement and atomically reserve an empty structural slot.
    fn reserve_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError>;
    /// Check placement and the exact reservation without changing it.
    fn validate_construction(
        &self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError>;
    /// Release only the matching reservation, including when terrain has changed.
    fn cancel_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError>;
    /// Validate placement and atomically replace the matching reservation.
    /// Reject a building belonging to another settlement.
    fn finish_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
        building: BuildingReference,
    ) -> Result<(), TerritoryError>;
    /// Validate placement and register a building only on an empty slot.
    fn register_building(
        &mut self,
        position: Position,
        building: BuildingReference,
    ) -> Result<(), TerritoryError>;
    /// Release a rejected project's reservation, preserving any other occupant.
    fn release_rejected_construction(&mut self, position: Position, project: ProjectReference);
}
