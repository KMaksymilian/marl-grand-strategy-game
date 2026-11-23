//! Building registration, activity, utilization and productivity settings.
use super::super::{BuildingCommandError, Settlement};
use crate::domain::buildings::Building;
use crate::domain::buildings::definition::{BuildingBehavior, BuildingDefinition};
use crate::domain::buildings::productivity::BuildingProductivityCalculator;
use crate::domain::buildings::productivity::{ProductionModifierStrategy, TravelCostSource};
use crate::domain::economy::ResourceType;
use crate::domain::ids::BuildingId;
use crate::domain::spatial::{BuildingReference, ConstructionSpace, Position, TerritoryError};

impl Settlement {
    /// Adds a yield modifier source for this settlement's production turns.
    pub fn add_production_modifier_strategy(
        &mut self,
        strategy: impl ProductionModifierStrategy + 'static,
    ) {
        self.building_manager
            .add_production_modifier_strategy(strategy);
    }

    pub(crate) fn validate_coastal_site(
        definition: &BuildingDefinition,
        position: Position,
        space: &impl ConstructionSpace,
    ) -> Result<(), TerritoryError> {
        if definition.requires_coast() && !space.is_coastal(position) {
            return Err(TerritoryError::CoastRequired(position));
        }
        Ok(())
    }

    pub fn first_unknown_building_material(
        &self,
        definition: &BuildingDefinition,
    ) -> Option<ResourceType> {
        let recipes = definition
            .behaviors()
            .iter()
            .flat_map(|behavior| match behavior {
                BuildingBehavior::Production { inputs, outputs } => inputs.iter().chain(outputs),
                BuildingBehavior::StatModifier { .. } => [].iter().chain(&[]),
            });
        recipes
            .map(|(resource, _)| resource)
            .chain(definition.construction_cost().needed_resources.keys())
            .chain(definition.deposit_resource())
            .filter(|resource| !self.knows_material(resource))
            .min()
            .cloned()
    }

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
        if let Some(resource) = self.first_unknown_building_material(building.definition()) {
            return Err(BuildingCommandError::UnknownMaterial(resource));
        }
        Self::validate_coastal_site(building.definition(), position, environment)
            .map_err(BuildingCommandError::Territory)?;
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
        let calculator = calculator.map(|calculator| {
            self.capital().map_or(calculator, |capital| {
                BuildingProductivityCalculator::new(capital, calculator.policy())
            })
        });
        self.building_manager
            .set_productivity_calculator(calculator, source);
    }
}
