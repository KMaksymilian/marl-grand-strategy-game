use crate::domain::economy::resource::{ResourceInventory, ResourceType};
use std::collections::HashMap;

/// Sequence define strength of priority (descending)
// #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
// pub enum Priority {
//     Luxury,
//     Comfort,
//     Industry,
//     Military,
//     Survival,
//     CriticalOverride,
// }

#[derive(Debug, Clone)]
pub struct DemandRequest {
    pub entity_id: u32, // 0 for population
    pub demand: HashMap<ResourceType, u32>,
}

pub type AllocationResult = HashMap<u32, HashMap<ResourceType, u32>>;

pub struct AllocationEngine;

impl AllocationEngine {
    pub fn execute(
        inventory: &mut ResourceInventory,
        requests: Vec<DemandRequest>,
    ) -> AllocationResult {
        let mut final_allocations: AllocationResult = HashMap::new();

        // 1. Calculating global demand
        let mut total_demand: HashMap<ResourceType, u32> = HashMap::new();
        for req in &requests {
            for (res, amount) in &req.demand {
                *total_demand.entry(*res).or_insert(0) += amount;
            }
        }

        // 2. Calculating fulfillment for every demand
        let mut global_ratios: HashMap<ResourceType, f32> = HashMap::new();
        for (res, total_needed) in total_demand {
            if total_needed > 0 {
                let available = inventory.resources.get(&res).copied().unwrap_or(0);
                let ratio = (available as f32 / total_needed as f32).min(1.0);
                global_ratios.insert(res, ratio);
            }
        }

        // 3. Allocating resources
        for req in requests {
            for (res, needed) in req.demand {
                let ratio = global_ratios.get(&res).copied().unwrap_or(0.0);
                let allocated_amount = (needed as f32 * ratio).floor() as u32;

                if allocated_amount > 0 {
                    inventory.consume(res, allocated_amount);

                    *final_allocations
                        .entry(req.entity_id)
                        .or_default()
                        .entry(res)
                        .or_insert(0) += allocated_amount;
                }
            }
        }

        final_allocations
    }
}


