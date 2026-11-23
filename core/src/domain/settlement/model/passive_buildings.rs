//! Completed building effects and settlement-wide building rules.
use super::economy::BuildingManager;
use super::projects::{Project, ProjectError, ProjectResult};
use super::{BuildingCommandError, Settlement};
use crate::domain::buildings::definition::BuildingDefinition;
use crate::domain::buildings::passive::{
    BuildingRuleError, BuildingUpgradeRule, PassiveBuildingEffects, TradeLimitBonuses,
};
use crate::domain::ids::BuildingId;

impl Settlement {
    /// Passive effects are supplied by completed buildings regardless of staffing.
    pub fn passive_building_effects(&self) -> PassiveBuildingEffects {
        let mut effects = PassiveBuildingEffects::default();
        for building in self.buildings() {
            effects.add(building.definition().passive_rules().effects);
        }
        effects
    }

    pub fn trade_limit_bonuses(&self) -> TradeLimitBonuses {
        self.passive_building_effects().trade
    }

    pub fn monument_happiness_bonus(&self) -> u32 {
        self.passive_building_effects().happiness
    }

    pub fn fortification_defense_basis_points(&self) -> u32 {
        self.passive_building_effects().defense_basis_points
    }

    pub(super) fn refresh_warehouse_capacity(&mut self) {
        let bonus = self.passive_building_effects().warehouse_capacity;
        self.inventory_manager.apply_building_capacity(bonus);
    }

    pub(crate) fn validate_new_building_rules(
        &self,
        definition: &BuildingDefinition,
    ) -> Result<(), BuildingRuleError> {
        validate_building_rules(
            &self.building_manager,
            self.active_projects(),
            definition,
            None,
        )
    }

    pub(crate) fn validate_building_upgrade_rules(
        &self,
        target: BuildingId,
        definition: &BuildingDefinition,
    ) -> Result<(), ProjectError> {
        if self.active_projects().iter().any(|project| {
            matches!(project.result(), ProjectResult::UpgradeBuilding { target_instance_id, .. } if *target_instance_id == target)
        }) {
            return Err(ProjectError::BuildingAlreadyUpgrading(target));
        }
        let current = self
            .building_manager
            .get(target)
            .ok_or(ProjectError::MissingBuilding(target))?;
        validate_building_rules(
            &self.building_manager,
            self.active_projects(),
            definition,
            Some(current),
        )
        .map_err(ProjectError::BuildingRule)
    }

    pub(crate) fn remove_demolished_building(
        &mut self,
        id: BuildingId,
    ) -> Result<(), BuildingCommandError> {
        let index = self
            .building_manager
            .buildings
            .iter()
            .position(|building| building.instance_id() == id)
            .ok_or(BuildingCommandError::MissingBuilding(id))?;
        self.building_manager.buildings.remove(index);
        self.project_manager.active_projects.retain(|project| {
            !matches!(project.result(), ProjectResult::UpgradeBuilding { target_instance_id, .. } if *target_instance_id == id)
        });
        self.refresh_warehouse_capacity();
        Ok(())
    }
}

pub(super) fn validate_building_rules(
    buildings: &BuildingManager,
    projects: &[Project],
    definition: &BuildingDefinition,
    replacing: Option<&crate::domain::buildings::Building>,
) -> Result<(), BuildingRuleError> {
    let rules = definition.passive_rules();
    match &rules.upgrade {
        BuildingUpgradeRule::ConstructionOnly if replacing.is_some() => {
            return Err(BuildingRuleError::ConstructionOnly {
                building_type: definition.id().to_owned(),
            });
        }
        BuildingUpgradeRule::From(previous_type)
            if replacing.is_none_or(|building| building.definition().id() != previous_type) =>
        {
            return Err(BuildingRuleError::UpgradeRequires {
                previous_type: previous_type.clone(),
            });
        }
        _ => {}
    }
    if let Some(limit) = &rules.limit {
        let built = buildings
            .buildings
            .iter()
            .filter(|building| {
                replacing.is_none_or(|replaced| replaced.instance_id() != building.instance_id())
                    && in_group(building.definition(), &limit.group)
            })
            .count();
        let pending = projects
            .iter()
            .filter(|project| match project.result() {
                ProjectResult::ConstructBuilding { definition, .. } => {
                    in_group(definition, &limit.group)
                }
                ProjectResult::UpgradeBuilding {
                    target_instance_id,
                    new_definition,
                } => {
                    in_group(new_definition, &limit.group)
                        && !buildings
                            .get(*target_instance_id)
                            .is_some_and(|building| in_group(building.definition(), &limit.group))
                }
                ProjectResult::RecruitUnit { .. } => false,
            })
            .count();
        if built.saturating_add(pending) >= limit.maximum as usize {
            return Err(BuildingRuleError::LimitReached {
                group: limit.group.clone(),
                maximum: limit.maximum,
            });
        }
    }
    Ok(())
}

fn in_group(definition: &BuildingDefinition, group: &str) -> bool {
    definition
        .passive_rules()
        .limit
        .as_ref()
        .is_some_and(|limit| limit.group == group)
}
