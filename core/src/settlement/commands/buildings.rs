//! Building registration, activity, utilization and productivity settings.
use super::super::{BuildingCommandError, Settlement};
use crate::buildings::Building;
use crate::buildings::productivity::BuildingProductivityCalculator;
use crate::ids::BuildingId;
use crate::world::map::Map;
use crate::world::pathfinding::path_finder::PathFinder;
use crate::world::territory::{BuildingReference, TerritoryManager};

impl Settlement {
    pub fn add_building(
        &mut self,
        building: Building,
        map: &Map,
        finder: &mut PathFinder,
        territory: &mut TerritoryManager,
    ) -> Result<(), BuildingCommandError> {
        let id = building.instance_id();
        let position = building.position();
        if self.building_manager.contains(id) {
            return Err(BuildingCommandError::DuplicateBuilding(id));
        }
        map.validate_building_position(position)
            .map_err(BuildingCommandError::Territory)?;
        territory
            .register_building(
                position,
                BuildingReference {
                    settlement_id: self.id(),
                    building_id: id,
                },
            )
            .map_err(BuildingCommandError::Territory)?;
        // The duplicate check above makes registration infallible under this exclusive borrow.
        self.building_manager
            .construct_building(building, map, finder)
            .expect("building ID was checked before spatial registration");
        Ok(())
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
