//! Short-lived adapters. Lifetimes belong here, never to Settlement or Building.
use super::pathfinding::{a_star::AStar, path_finder::PathFinder};
use super::{
    Map,
    territory::{
        BuildingReference, ProjectReference, TerritoryError, TerritoryManager, TileOccupancy,
    },
};
use crate::domain::buildings::productivity::TravelCostSource;
use crate::domain::spatial::{ConstructionSpace, Position};

pub struct AStarTravelCosts<'a> {
    map: &'a Map,
    finder: &'a mut PathFinder,
}

impl<'a> AStarTravelCosts<'a> {
    pub fn new(map: &'a Map, finder: &'a mut PathFinder) -> Self {
        Self { map, finder }
    }
}

impl TravelCostSource for AStarTravelCosts<'_> {
    fn travel_cost(&mut self, from: Position, to: Position) -> Option<u32> {
        let start = (usize::try_from(from.x).ok()?, usize::try_from(from.y).ok()?);
        let end = (usize::try_from(to.x).ok()?, usize::try_from(to.y).ok()?);
        AStar::travel_cost(self.map, self.finder, start, end)
    }
}

pub struct MapConstructionSpace<'a> {
    map: &'a Map,
    territory: &'a mut TerritoryManager,
}

impl<'a> MapConstructionSpace<'a> {
    pub fn new(map: &'a Map, territory: &'a mut TerritoryManager) -> Self {
        Self { map, territory }
    }
}

impl ConstructionSpace for MapConstructionSpace<'_> {
    fn reserve_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError> {
        self.map.validate_building_position(position)?;
        self.territory.reserve_construction(position, project)
    }
    fn validate_construction(
        &self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError> {
        self.map.validate_building_position(position)?;
        self.territory.validate_reservation(position, project)
    }
    fn cancel_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError> {
        self.territory.cancel_construction(position, project)
    }
    fn finish_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
        building: BuildingReference,
    ) -> Result<(), TerritoryError> {
        self.map.validate_building_position(position)?;
        self.territory
            .finish_construction(position, project, building)
    }
    fn register_building(
        &mut self,
        position: Position,
        building: BuildingReference,
    ) -> Result<(), TerritoryError> {
        self.map.validate_building_position(position)?;
        self.territory.register_building(position, building)
    }
    fn release_rejected_construction(&mut self, position: Position, project: ProjectReference) {
        if self.territory.tile_at(position).occupancy() == TileOccupancy::Construction(project) {
            self.territory
                .cancel_construction(position, project)
                .expect("matching reservation");
        }
    }
}

/// Composition of two narrow ports for commands/phases requiring both capabilities.
pub struct MapSettlementEnvironment<'a> {
    space: MapConstructionSpace<'a>,
    costs: AStarTravelCosts<'a>,
}

impl<'a> MapSettlementEnvironment<'a> {
    pub fn new(
        map: &'a Map,
        finder: &'a mut PathFinder,
        territory: &'a mut TerritoryManager,
    ) -> Self {
        Self {
            space: MapConstructionSpace::new(map, territory),
            costs: AStarTravelCosts::new(map, finder),
        }
    }
}

impl TravelCostSource for MapSettlementEnvironment<'_> {
    fn travel_cost(&mut self, from: Position, to: Position) -> Option<u32> {
        self.costs.travel_cost(from, to)
    }
}

impl ConstructionSpace for MapSettlementEnvironment<'_> {
    fn reserve_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError> {
        self.space.reserve_construction(position, project)
    }
    fn validate_construction(
        &self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError> {
        self.space.validate_construction(position, project)
    }
    fn cancel_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError> {
        self.space.cancel_construction(position, project)
    }
    fn finish_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
        building: BuildingReference,
    ) -> Result<(), TerritoryError> {
        self.space.finish_construction(position, project, building)
    }
    fn register_building(
        &mut self,
        position: Position,
        building: BuildingReference,
    ) -> Result<(), TerritoryError> {
        self.space.register_building(position, building)
    }
    fn release_rejected_construction(&mut self, position: Position, project: ProjectReference) {
        self.space.release_rejected_construction(position, project);
    }
}
