//! Territory and natural deposits derive from the authoritative map layers.
use super::{World, WorldError};
use crate::domain::economy::ResourceType;
use crate::domain::ids::SettlementId;
use crate::domain::world::Position;
use std::collections::hash_map::Entry;

impl World {
    /// Controlled tiles in row-major order, including water within the province.
    pub fn controlled_positions(&self, id: SettlementId) -> Result<Vec<Position>, WorldError> {
        let province = self
            .settlement_province(id)
            .ok_or(WorldError::MissingSettlement(id))?;
        Ok(self.province_positions(province))
    }

    /// Scenario setup: place one permanent material deposit on a land tile.
    pub fn place_deposit(
        &mut self,
        position: Position,
        resource: ResourceType,
    ) -> Result<(), WorldError> {
        self.province_at(position)?;
        if self.map.validate_building_position(position).is_err() {
            return Err(WorldError::DepositOnOcean(position));
        }
        if !ResourceType::MATERIALS.contains(&resource) {
            return Err(WorldError::InvalidDepositResource(resource));
        }
        match self.map.deposits.entry(position) {
            Entry::Vacant(entry) => {
                entry.insert(resource);
                Ok(())
            }
            Entry::Occupied(_) => Err(WorldError::DepositAlreadyExists(position)),
        }
    }

    pub fn accessible_deposits(
        &self,
        id: SettlementId,
    ) -> Result<Vec<(Position, ResourceType)>, WorldError> {
        Ok(self
            .controlled_positions(id)?
            .into_iter()
            .filter_map(|position| {
                let resource = self.map.deposit_at(position)?;
                self.map.validate_building_position(position).ok()?;
                Some((position, resource))
            })
            .collect())
    }

    pub(super) fn validate_building_deposit(
        &self,
        position: Position,
        required: Option<ResourceType>,
    ) -> Result<(), WorldError> {
        if let Some(resource) = required {
            let province = self.province_at(position)?;
            if !self
                .map
                .has_accessible_deposit(province, position, resource)
            {
                return Err(WorldError::RequiredDepositMissing { position, resource });
            }
        }
        Ok(())
    }
}
