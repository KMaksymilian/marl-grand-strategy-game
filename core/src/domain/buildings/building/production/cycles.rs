use super::{
    Building, ProductionModifierStrategy, ProductionResult, ResourceType, TurnProductionModifiers,
};
use crate::domain::buildings::definition::{CycleProduction, CycleRecipe};
use std::cmp::Reverse;
use std::collections::HashMap;

impl Building {
    pub(crate) fn preferred_cycle_recipe(
        &self,
        available: impl Fn(&ResourceType) -> u32,
    ) -> Option<(usize, u32)> {
        self.choose_cycle_recipe(self.requested_workers(), available)
    }

    pub(super) fn choose_cycle_recipe(
        &self,
        workers: u32,
        available: impl Fn(&ResourceType) -> u32,
    ) -> Option<(usize, u32)> {
        let production = self.definition.cycle_production()?;
        self.configured_biome_efficiency()?;
        let staff_limit = self.staffed_cycles(production, workers);
        production
            .recipes
            .iter()
            .enumerate()
            .map(|(index, recipe)| (index, executable_cycles(recipe, staff_limit, &available)))
            .filter(|&(_, cycles)| cycles > 0)
            .max_by_key(|&(index, cycles)| (cycles, Reverse(index)))
    }

    pub(crate) fn calculate_cycle_demand(
        &self,
        plan: Option<(usize, u32)>,
    ) -> HashMap<ResourceType, u32> {
        let mut demand = HashMap::new();
        let workers = self.requested_workers();
        if workers == 0 || self.configured_biome_efficiency().is_none() {
            return demand;
        }

        demand.insert(ResourceType::Labour, workers);
        if let Some(production) = self.definition.cycle_production()
            && let Some((index, cycles)) = plan
            && let Some(recipe) = production.recipes.get(index)
        {
            for (resource, units) in &recipe.inputs {
                demand.insert(resource.clone(), cycles * units);
            }
        }
        demand
    }

    pub(super) fn execute_cycles(
        &self,
        production: &CycleProduction,
        recipe_index: Option<usize>,
        mut unused: HashMap<ResourceType, u32>,
        modifier_strategies: &[Box<dyn ProductionModifierStrategy>],
        turn_modifiers: TurnProductionModifiers<'_>,
    ) -> ProductionResult {
        let mut produced = HashMap::new();
        if let Some(recipe) = recipe_index.and_then(|index| production.recipes.get(index))
            && let Some(efficiency) = self.configured_biome_efficiency()
        {
            let staff_limit = self.staffed_cycles(production, self.current_workers);
            let cycles = executable_cycles(recipe, staff_limit, |resource| {
                unused.get(resource).copied().unwrap_or_default()
            });
            if cycles > 0 {
                for (resource, units) in &recipe.inputs {
                    // Cycle count is bounded by every granted input.
                    *unused
                        .get_mut(resource)
                        .expect("cycle input must be granted") -= cycles * units;
                }
                let production_yield =
                    self.production_yield(efficiency, modifier_strategies, turn_modifiers.global);
                produced.extend(recipe.outputs.iter().map(|(resource, units)| {
                    (
                        resource.clone(),
                        production_yield.scale_output(
                            *units,
                            u128::from(cycles),
                            1,
                            turn_modifiers.material_multiplier(resource),
                        ),
                    )
                }));
            }
        }
        ProductionResult { produced, unused }
    }

    fn configured_biome_efficiency(&self) -> Option<u8> {
        self.production_biome
            .map_or(Some(100), |biome| self.definition.biome_efficiency(biome))
    }

    fn staffed_cycles(&self, production: &CycleProduction, workers: u32) -> u32 {
        (u64::from(production.cycles_per_turn) * u64::from(workers))
            .checked_div(u64::from(self.definition.max_workers()))
            .unwrap_or_default() as u32
    }
}

fn executable_cycles(
    recipe: &CycleRecipe,
    staff_limit: u32,
    available: impl Fn(&ResourceType) -> u32,
) -> u32 {
    recipe
        .inputs
        .iter()
        .map(|(resource, units)| available(resource) / units)
        .fold(staff_limit, u32::min)
}
