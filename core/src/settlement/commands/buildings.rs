//! Building registration, activity, utilization and productivity settings.
use super::super::{BuildingCommandError, Settlement};
use crate::buildings::Building;
use crate::buildings::productivity::BuildingProductivityCalculator;
use crate::ids::BuildingId;
use crate::world::map::Map;
use crate::world::pathfinding::path_finder::PathFinder;

impl Settlement {
    pub fn add_building(
        &mut self,
        building: Building,
        map: &Map,
        finder: &mut PathFinder,
    ) -> Result<(), BuildingId> {
        self.building_manager
            .construct_building(building, map, finder)
    }

    pub fn set_building_load(
        &mut self,
        id: BuildingId,
        value: f32,
    ) -> Result<(), BuildingCommandError> {
        self.building_manager
            .buildings
            .iter_mut()
            .find(|building| building.instance_id() == id)
            .ok_or(BuildingCommandError::MissingBuilding(id))?
            .set_target_load(value)
            .map_err(|_| BuildingCommandError::InvalidLoad)
    }

    pub fn set_building_active(
        &mut self,
        id: BuildingId,
        active: bool,
    ) -> Result<(), BuildingCommandError> {
        self.building_manager
            .buildings
            .iter_mut()
            .find(|building| building.instance_id() == id)
            .ok_or(BuildingCommandError::MissingBuilding(id))?
            .set_active(active);
        Ok(())
    }

    /// Recomputes cached multipliers immediately; `None` restores full productivity.
    pub fn set_building_productivity_calculator(
        &mut self,
        calculator: Option<BuildingProductivityCalculator>,
        map: &Map,
        finder: &mut PathFinder,
    ) {
        self.building_manager
            .set_productivity_calculator(calculator, map, finder);
    }
}
