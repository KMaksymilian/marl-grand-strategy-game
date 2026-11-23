//! Input/labour demand, allocation resolution and exact recipe execution.
mod cycles;

use super::{Building, ProductionResult};
use crate::domain::buildings::definition::BuildingBehavior;
use crate::domain::buildings::productivity::{
    ProductionModifier, ProductionModifierContext, ProductionModifierStrategy, ProductionYield,
};
use crate::domain::economy::ResourceType;
use std::collections::HashMap;

impl Building {
    pub fn calculate_demand(&self) -> HashMap<ResourceType, u32> {
        if self.definition.cycle_production().is_some() {
            return self.calculate_cycle_demand(self.preferred_cycle_recipe(|_| u32::MAX));
        }

        let mut demand = HashMap::new();

        if !self.is_active {
            return demand;
        }

        let labor_needed = self.requested_workers();
        if labor_needed > 0 {
            demand.insert(ResourceType::Labour, labor_needed);
        }

        for behavior in self.definition.behaviors() {
            if let BuildingBehavior::Production { inputs, .. } = behavior {
                for (res, amount) in inputs {
                    *demand.entry(res.clone()).or_insert(0) +=
                        (*amount as f64 * f64::from(self.target_load)).floor() as u32;
                }
            }
        }
        demand
    }

    pub fn resolve_allocation(
        &mut self,
        granted: Option<&HashMap<ResourceType, u32>>,
    ) -> ProductionResult {
        self.resolve_modified_allocation(granted, &[])
    }

    pub(crate) fn resolve_modified_allocation(
        &mut self,
        granted: Option<&HashMap<ResourceType, u32>>,
        modifier_strategies: &[Box<dyn ProductionModifierStrategy>],
    ) -> ProductionResult {
        let recipe = granted.and_then(|grants| {
            let workers = grants
                .get(&ResourceType::Labour)
                .copied()
                .unwrap_or_default()
                .min(self.definition.max_workers());
            self.choose_cycle_recipe(workers, |resource| {
                grants.get(resource).copied().unwrap_or_default()
            })
            .map(|(index, _)| index)
        });
        self.resolve_planned_allocation(granted, modifier_strategies, recipe)
    }

    pub(crate) fn resolve_planned_allocation(
        &mut self,
        granted: Option<&HashMap<ResourceType, u32>>,
        modifier_strategies: &[Box<dyn ProductionModifierStrategy>],
        cycle_recipe: Option<usize>,
    ) -> ProductionResult {
        let mut material_grants = granted.cloned().unwrap_or_default();

        let labor = material_grants.remove(&ResourceType::Labour).unwrap_or(0);
        if self.is_active {
            self.current_workers = labor.min(self.definition.max_workers());
        } else {
            self.current_workers = 0;
        }

        if let Some(production) = self.definition.cycle_production() {
            return self.execute_cycles(
                production,
                cycle_recipe,
                material_grants,
                modifier_strategies,
            );
        }
        self.execute_production(material_grants, modifier_strategies)
    }

    fn production_yield(
        &self,
        biome_efficiency: u8,
        modifier_strategies: &[Box<dyn ProductionModifierStrategy>],
    ) -> ProductionYield {
        let context = ProductionModifierContext {
            building: self,
            biome: self.production_biome,
        };
        let biome_modifier = (biome_efficiency != 100)
            .then(|| ProductionModifier::new("biome", (i32::from(biome_efficiency) - 100) * 100));
        self.location_productivity.apply_modifiers(
            biome_modifier.into_iter().chain(
                modifier_strategies
                    .iter()
                    .filter_map(|strategy| strategy.modifier(&context)),
            ),
        )
    }

    fn execute_production(
        &self,
        mut unused: HashMap<ResourceType, u32>,
        modifier_strategies: &[Box<dyn ProductionModifierStrategy>],
    ) -> ProductionResult {
        let mut produced = HashMap::new();

        if self.current_workers == 0 {
            return ProductionResult { produced, unused };
        }

        let production_yield = self.production_yield(100, modifier_strategies);

        // Recipes run in definition order against the remaining material balance.
        // Exact fractions avoid floating-point rounding at the withdrawal boundary.
        for behavior in self.definition.behaviors() {
            if let BuildingBehavior::Production { inputs, outputs } = behavior {
                let mut required: HashMap<&ResourceType, u64> = HashMap::new();
                for (resource, amount) in inputs {
                    *required.entry(resource).or_default() += u64::from(*amount);
                }
                let mut numerator = u128::from(self.current_workers);
                let mut denominator = u128::from(self.definition.max_workers());
                for (&resource, &amount) in &required {
                    let available = u128::from(unused.get(resource).copied().unwrap_or(0));
                    if amount > 0 && available * denominator < u128::from(amount) * numerator {
                        numerator = available;
                        denominator = u128::from(amount);
                    }
                }
                if numerator == 0 {
                    continue;
                }
                // Aggregate repeated inputs before rounding so each resource is
                // withdrawn once, never exceeding its remaining grant.
                for (resource, amount) in required {
                    let consumed = (u128::from(amount) * numerator).div_ceil(denominator);
                    if consumed > 0 {
                        let remaining = unused.get_mut(resource).expect("input must be granted");
                        *remaining = remaining
                            .checked_sub(
                                u32::try_from(consumed).expect("consumption cannot exceed grant"),
                            )
                            .expect("recipe cannot consume more than its remaining grant");
                    }
                }
                for (resource, amount) in outputs {
                    let output = production_yield.scale_output(*amount, numerator, denominator);
                    let total = produced.entry(resource.clone()).or_insert(0_u64);
                    *total = total.saturating_add(output);
                }
            }
        }
        ProductionResult { produced, unused }
    }
}
