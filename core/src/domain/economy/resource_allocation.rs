use crate::domain::economy::resource::{ResourceInventory, ResourceType};
use std::collections::HashMap;

/// Sequence define strength of priority (descending)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Luxury,
    Comfort,
    Industry,
    Military,
    Survival,
    CriticalOverride,
}

#[derive(Debug, Clone)]
pub struct DemandRequest {
    pub entity_id: u32, // 0 for population
    pub priority: Priority,
    pub demand: HashMap<ResourceType, f32>,
}

pub type AllocationResult = HashMap<u32, HashMap<ResourceType, f32>>;

pub struct AllocationEngine;

impl AllocationEngine {
    pub fn execute(
        inventory: &mut ResourceInventory,
        mut requests: Vec<DemandRequest>,
    ) -> AllocationResult {
        let mut final_allocations: AllocationResult = HashMap::new();

        requests.sort_by_key(|b| std::cmp::Reverse(b.priority));

        let mut grouped_requests: Vec<Vec<DemandRequest>> = Vec::new();
        for req in requests {
            if let Some(last_group) = grouped_requests.last_mut()
                && last_group.first().unwrap().priority == req.priority
            {
                last_group.push(req);
                continue;
            }
            grouped_requests.push(vec![req]);
        }

        for group in grouped_requests {
            let mut group_demand: HashMap<ResourceType, f32> = HashMap::new();
            for req in &group {
                for (res, amount) in &req.demand {
                    *group_demand.entry(*res).or_insert(0.0) += amount;
                }
            }

            let mut group_ratios: HashMap<ResourceType, f32> = HashMap::new();
            for (res, total_needed) in group_demand {
                if total_needed > 0.0 {
                    let available = inventory.resources.get(&res).copied().unwrap_or(0.0);
                    let ratio = (available / total_needed).min(1.0);
                    group_ratios.insert(res, ratio);
                }
            }

            for req in group {
                for (res, needed) in req.demand {
                    let ratio = group_ratios.get(&res).copied().unwrap_or(0.0);
                    let allocated_amount = needed * ratio;

                    if allocated_amount > 0.0 {
                        inventory.consume(res, allocated_amount);

                        *final_allocations
                            .entry(req.entity_id)
                            .or_default()
                            .entry(res)
                            .or_insert(0.0) += allocated_amount;
                    }
                }
            }
        }

        final_allocations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to quickly create single-resource demand requests
    fn create_request(
        entity_id: u32,
        priority: Priority,
        resource: ResourceType,
        amount: f32,
    ) -> DemandRequest {
        let mut demand = HashMap::new();
        demand.insert(resource, amount);
        DemandRequest {
            entity_id,
            priority,
            demand,
        }
    }

    #[test]
    fn test_allocation_full_satisfaction() {
        let mut inventory = ResourceInventory::new(100.0);
        inventory.add(ResourceType::Wood, 50.0);

        let req1 = create_request(1, Priority::Industry, ResourceType::Wood, 20.0);
        let req2 = create_request(2, Priority::Survival, ResourceType::Wood, 10.0);

        let requests = vec![req1, req2];
        let result = AllocationEngine::execute(&mut inventory, requests);

        // Both entities should get exactly what they asked for (100% satisfaction)
        assert_eq!(
            *result.get(&1).unwrap().get(&ResourceType::Wood).unwrap(),
            20.0
        );
        assert_eq!(
            *result.get(&2).unwrap().get(&ResourceType::Wood).unwrap(),
            10.0
        );

        // 30 wood was consumed, 20 should be left in the inventory
        assert_eq!(
            *inventory.resources.get(&ResourceType::Wood).unwrap_or(&0.0),
            20.0
        );
    }

    #[test]
    fn test_allocation_strict_priority_enforcement() {
        let mut inventory = ResourceInventory::new(100.0);
        inventory.add(ResourceType::Wood, 10.0); // Only 10 wood available

        // Industry wants 10, but Survival also wants 10.
        let req_industry = create_request(1, Priority::Industry, ResourceType::Wood, 10.0);
        let req_survival = create_request(2, Priority::Survival, ResourceType::Wood, 10.0);

        // Order in the vector shouldn't matter; the engine must sort them by priority
        let requests = vec![req_industry, req_survival];
        let result = AllocationEngine::execute(&mut inventory, requests);

        // Survival (higher priority) takes everything
        assert_eq!(
            *result.get(&2).unwrap().get(&ResourceType::Wood).unwrap(),
            10.0
        );

        // Industry gets absolutely nothing
        let industry_allocation = result.get(&1).cloned().unwrap_or_default();
        assert_eq!(
            *industry_allocation.get(&ResourceType::Wood).unwrap_or(&0.0),
            0.0
        );

        // Inventory should be empty
        assert_eq!(
            *inventory.resources.get(&ResourceType::Wood).unwrap_or(&0.0),
            0.0
        );
    }

    #[test]
    fn test_allocation_proportional_sharing_within_same_priority() {
        let mut inventory = ResourceInventory::new(100.0);
        inventory.add(ResourceType::Wood, 10.0); // Only 10 wood available

        // Two factories (same priority), each wants 10 wood (20 total demand)
        let req_factory_a = create_request(1, Priority::Industry, ResourceType::Wood, 10.0);
        let req_factory_b = create_request(2, Priority::Industry, ResourceType::Wood, 10.0);

        let requests = vec![req_factory_a, req_factory_b];
        let result = AllocationEngine::execute(&mut inventory, requests);

        // The fulfillment ratio for this priority group is 10/20 = 0.5 (50%)
        // Both factories should receive exactly 5 wood
        assert_eq!(
            *result.get(&1).unwrap().get(&ResourceType::Wood).unwrap(),
            5.0
        );
        assert_eq!(
            *result.get(&2).unwrap().get(&ResourceType::Wood).unwrap(),
            5.0
        );

        // Inventory should be empty
        assert_eq!(
            *inventory.resources.get(&ResourceType::Wood).unwrap_or(&0.0),
            0.0
        );
    }

    #[test]
    fn test_allocation_weighted_proportional_sharing() {
        let mut inventory = ResourceInventory::new(100.0);
        inventory.add(ResourceType::Wood, 15.0);

        // Big factory wants 20, small factory wants 10. Total demand = 30.
        // Available is 15, so the ratio is 15/30 = 0.5.
        let req_big = create_request(1, Priority::Industry, ResourceType::Wood, 20.0);
        let req_small = create_request(2, Priority::Industry, ResourceType::Wood, 10.0);

        let requests = vec![req_big, req_small];
        let result = AllocationEngine::execute(&mut inventory, requests);

        // Big factory gets 50% of 20 = 10
        assert_eq!(
            *result.get(&1).unwrap().get(&ResourceType::Wood).unwrap(),
            10.0
        );
        // Small factory gets 50% of 10 = 5
        assert_eq!(
            *result.get(&2).unwrap().get(&ResourceType::Wood).unwrap(),
            5.0
        );
    }

    #[test]
    fn test_allocation_aggregation_for_same_entity() {
        let mut inventory = ResourceInventory::new(100.0);
        inventory.add(ResourceType::Grain, 15.0);

        // Population (entity_id = 0) sends two requests from different Need Tiers
        let req_survival = create_request(0, Priority::Survival, ResourceType::Grain, 10.0);
        let req_comfort = create_request(0, Priority::Comfort, ResourceType::Grain, 10.0);

        // A brewery (Industry) sits between the population's priorities and wants 10 Grain
        let req_industry = create_request(1, Priority::Industry, ResourceType::Grain, 10.0);

        let requests = vec![req_survival, req_comfort, req_industry];
        let result = AllocationEngine::execute(&mut inventory, requests);

        // The Waterfall execution order:
        // 1. Survival (id=0) takes 10. (5 remaining)
        // 2. Industry (id=1) takes 5. (0 remaining)
        // 3. Comfort (id=0) takes 0.

        // Population (id=0) should have its results AGGREGATED into one HashMap: 10 (from survival) + 0 (from comfort)
        assert_eq!(
            *result.get(&0).unwrap().get(&ResourceType::Grain).unwrap(),
            10.0
        );

        // Industry gets the leftovers (5)
        assert_eq!(
            *result.get(&1).unwrap().get(&ResourceType::Grain).unwrap(),
            5.0
        );
    }

    #[test]
    fn test_allocation_multiple_resources_in_single_request() {
        let mut inventory = ResourceInventory::new(100.0);
        inventory.add(ResourceType::Wood, 10.0);
        inventory.add(ResourceType::Stone, 5.0);

        let mut demand = HashMap::new();
        demand.insert(ResourceType::Wood, 10.0);
        demand.insert(ResourceType::Stone, 10.0); // Stone is scarce

        let req = DemandRequest {
            entity_id: 1,
            priority: Priority::Industry,
            demand,
        };

        let result = AllocationEngine::execute(&mut inventory, vec![req]);

        // One resource is fully satisfied, the other only reaches 50%
        let allocated = result.get(&1).unwrap();
        assert_eq!(*allocated.get(&ResourceType::Wood).unwrap_or(&0.0), 10.0);
        assert_eq!(*allocated.get(&ResourceType::Stone).unwrap_or(&0.0), 5.0);
    }
}
