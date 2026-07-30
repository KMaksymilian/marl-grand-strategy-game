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
        let total_demand = Self::calculate_global_demand(&requests);
        let global_ratios = Self::calculate_fulfillment_ratios(inventory, &total_demand);

        Self::distribute_resources(inventory, requests, &global_ratios)
    }

    fn calculate_global_demand(requests: &[DemandRequest]) -> HashMap<ResourceType, u32> {
        let mut total_demand = HashMap::new();
        for req in requests {
            for (res, amount) in &req.demand {
                *total_demand.entry(*res).or_insert(0) += amount;
            }
        }
        total_demand
    }

    fn calculate_fulfillment_ratios(
        inventory: &ResourceInventory,
        total_demand: &HashMap<ResourceType, u32>,
    ) -> HashMap<ResourceType, f32> {
        let mut global_ratios = HashMap::new();
        for (res, total_needed) in total_demand {
            if *total_needed > 0 {
                let available = inventory.resources.get(res).copied().unwrap_or(0);
                let ratio = (available as f32 / *total_needed as f32).min(1.0);
                global_ratios.insert(*res, ratio);
            }
        }
        global_ratios
    }

    fn distribute_resources(
        inventory: &mut ResourceInventory,
        requests: Vec<DemandRequest>,
        global_ratios: &HashMap<ResourceType, f32>,
    ) -> AllocationResult {
        let mut final_allocations: AllocationResult = HashMap::new();

        for req in requests {
            for (res, needed) in req.demand {
                let ratio = global_ratios.get(&res).copied().unwrap_or(0.0);
                let allocated_amount = (needed as f32 * ratio).floor() as u32;

                if allocated_amount > 0 {
                    if res != ResourceType::Labour {
                        inventory.consume(res, allocated_amount);
                    }

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


