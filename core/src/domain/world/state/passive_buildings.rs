//! World commands and military queries for passive buildings.
use super::{World, WorldError};
use crate::domain::ids::{BuildingId, SettlementId};
use crate::domain::settlement::{BuildingCommandError, Settlement};
use crate::domain::spatial::BuildingReference;

impl World {
    /// Removes a completed building and its bonuses without refunding materials.
    pub fn demolish_building(
        &mut self,
        settlement_id: SettlementId,
        building_id: BuildingId,
    ) -> Result<(), WorldError> {
        let settlement = self
            .settlements
            .get_mut(settlement_id)
            .ok_or(WorldError::MissingSettlement(settlement_id))?;
        let position = settlement
            .buildings()
            .find(|building| building.instance_id() == building_id)
            .map(|building| building.position())
            .ok_or(WorldError::Building(BuildingCommandError::MissingBuilding(
                building_id,
            )))?;
        self.territory
            .remove_building(
                position,
                BuildingReference {
                    settlement_id,
                    building_id,
                },
            )
            .map_err(|error| WorldError::Building(BuildingCommandError::Territory(error)))?;
        settlement
            .remove_demolished_building(building_id)
            .expect("the building and its reference were just validated");
        Ok(())
    }

    /// Bonus to the defending garrison and army in this province.
    pub fn province_fortification_defense_basis_points(
        &self,
        province_id: usize,
    ) -> Result<u32, WorldError> {
        Ok(self
            .province_settlement(province_id)?
            .and_then(|id| self.settlement(id))
            .map_or(0, Settlement::fortification_defense_basis_points))
    }
}
