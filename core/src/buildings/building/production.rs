//! Input/labour demand, allocation resolution and exact recipe execution.
use super::{Building, BuildingUpdateResult, ProductionResult};
use crate::buildings::definition::BuildingBehavior;
use crate::economy::ResourceType;
use std::collections::HashMap;

impl Building {
    pub fn calculate_demand(&self) -> HashMap<ResourceType, u32> {
        let mut demand = HashMap::new();

        if !self.is_active {
            return demand;
        }

        let labor_needed =
            (self.definition.max_workers as f64 * f64::from(self.target_load)).floor() as u32;
        if labor_needed > 0 {
            demand.insert(ResourceType::Labour, labor_needed);
        }

        for behavior in &self.definition.behaviors {
            if let BuildingBehavior::Production { inputs, .. } = behavior {
                for (res, amount) in inputs {
                    *demand.entry(*res).or_insert(0) +=
                        (*amount as f64 * f64::from(self.target_load)).floor() as u32;
                }
            }
        }
        demand
    }

    pub fn resolve_allocation(
        &mut self,
        granted: Option<&HashMap<ResourceType, u32>>,
    ) -> BuildingUpdateResult {
        let mut material_grants = granted.cloned().unwrap_or_default();

        if self.is_active && self.definition.max_workers > 0 {
            let labor = material_grants.remove(&ResourceType::Labour).unwrap_or(0);
            self.current_workers = labor.min(self.definition.max_workers);
        } else {
            self.current_workers = 0;
            material_grants.remove(&ResourceType::Labour);
        }

        let production = self.execute_production(Some(&material_grants));
        // let modifiers = self.execute_stat_modification();

        BuildingUpdateResult {
            production,
            //  modifiers,
        }
    }

    fn execute_production(&self, granted: Option<&HashMap<ResourceType, u32>>) -> ProductionResult {
        let empty_grants = HashMap::new();
        let granted_ref = granted.unwrap_or(&empty_grants);

        let mut produced = HashMap::new();
        let mut unused = granted_ref.clone();

        if !self.is_active || self.current_workers == 0 {
            return ProductionResult { produced, unused };
        }

        // Recipes run in definition order against the remaining material balance.
        // Exact fractions avoid floating-point rounding at the withdrawal boundary.
        for behavior in &self.definition.behaviors {
            if let BuildingBehavior::Production { inputs, outputs } = behavior {
                let mut required: HashMap<ResourceType, u64> = HashMap::new();
                for &(resource, amount) in inputs {
                    *required.entry(resource).or_default() += u64::from(amount);
                }
                let mut numerator = u128::from(self.current_workers);
                let mut denominator = u128::from(self.definition.max_workers);
                for (&resource, &amount) in &required {
                    let available = u128::from(unused.get(&resource).copied().unwrap_or(0));
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
                        let remaining = unused.get_mut(&resource).expect("input must be granted");
                        *remaining = remaining
                            .checked_sub(
                                u32::try_from(consumed).expect("consumption cannot exceed grant"),
                            )
                            .expect("recipe cannot consume more than its remaining grant");
                    }
                }
                for &(resource, amount) in outputs {
                    let output =
                        self.location_productivity
                            .scale_output(amount, numerator, denominator);
                    let total = produced.entry(resource).or_insert(0_u32);
                    *total = total
                        .checked_add(output)
                        .expect("production exceeds u32 capacity");
                }
            }
        }
        ProductionResult { produced, unused }
    }
}
