//! Building registration, activity, utilization and productivity settings.
use super::super::{BuildingCommandError, Settlement};
use crate::domain::buildings::Building;
use crate::domain::buildings::productivity::BuildingProductivityCalculator;
use crate::domain::buildings::productivity::TravelCostSource;
use crate::domain::ids::BuildingId;
use crate::domain::spatial::{BuildingReference, ConstructionSpace};

impl Settlement {
    pub fn add_building(
        &mut self,
        building: Building,
        environment: &mut (impl ConstructionSpace + TravelCostSource),
    ) -> Result<(), BuildingCommandError> {
        let id = building.instance_id();
        let position = building.position();
        if self.building_manager.contains(id) {
            return Err(BuildingCommandError::DuplicateBuilding(id));
        }
        environment
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
            .construct_building(building, environment)
            .expect("building ID was checked before spatial registration");
        Ok(())
    }

    pub fn set_building_load(
        &mut self,
        id: BuildingId,
        value: f32,
    ) -> Result<(), BuildingCommandError> {
        self.building_manager
            .get_mut(id)
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
            .get_mut(id)
            .ok_or(BuildingCommandError::MissingBuilding(id))?
            .set_active(active);
        Ok(())
    }

    /// Activates the building and sets its load in one step; on error nothing changes.
    pub fn set_building_staff(
        &mut self,
        id: BuildingId,
        load: f32,
    ) -> Result<(), BuildingCommandError> {
        let building = self
            .building_manager
            .get_mut(id)
            .ok_or(BuildingCommandError::MissingBuilding(id))?;
        building
            .set_target_load(load)
            .map_err(|_| BuildingCommandError::InvalidLoad)?;
        building.set_active(true);
        Ok(())
    }

    /// Recomputes cached multipliers immediately; `None` restores full productivity.
    pub fn set_building_productivity_calculator(
        &mut self,
        calculator: Option<BuildingProductivityCalculator>,
        source: &mut impl TravelCostSource,
    ) {
        self.building_manager
            .set_productivity_calculator(calculator, source);
    }
}
