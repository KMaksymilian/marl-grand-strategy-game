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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::resource::{ResourceInventory, ResourceType};
    use std::collections::HashMap;

    // Helper method to quickly build requests
    fn create_request(entity_id: u32, demands: &[(ResourceType, u32)]) -> DemandRequest {
        let mut demand = HashMap::new();
        for &(res, amount) in demands {
            demand.insert(res, amount);
        }
        DemandRequest { entity_id, demand }
    }

    #[test]
    fn test_execute_full_allocation() {
        // Arrange
        let mut inventory = ResourceInventory::new(1000);
        inventory.add(ResourceType::Wood, 100);
        inventory.add(ResourceType::Stone, 50);

        let requests = vec![
            create_request(1, &[(ResourceType::Wood, 40), (ResourceType::Stone, 20)]),
            create_request(2, &[(ResourceType::Wood, 50)]),
        ];

        // Act
        let result = AllocationEngine::execute(&mut inventory, requests);

        // Assert
        // Entity 1 received exactly what it asked for
        assert_eq!(
            *result.get(&1).unwrap().get(&ResourceType::Wood).unwrap(),
            40
        );
        assert_eq!(
            *result.get(&1).unwrap().get(&ResourceType::Stone).unwrap(),
            20
        );

        // Entity 2 received exactly what it asked for
        assert_eq!(
            *result.get(&2).unwrap().get(&ResourceType::Wood).unwrap(),
            50
        );

        // Inventory is properly depleted
        assert_eq!(
            *inventory.resources.get(&ResourceType::Wood).unwrap(),
            10,
            "100 - (40 + 50) = 10"
        );
        assert_eq!(
            *inventory.resources.get(&ResourceType::Stone).unwrap(),
            30,
            "50 - 20 = 30"
        );
    }

    #[test]
    fn test_execute_partial_allocation_with_ratios() {
        // Arrange
        let mut inventory = ResourceInventory::new(1000);
        inventory.add(ResourceType::Wood, 100);

        // Total demand: 200 (E1 wants 150, E2 wants 50)
        // Ratio = 100 / 200 = 0.5 (everyone gets 50% of their request)
        let requests = vec![
            create_request(1, &[(ResourceType::Wood, 150)]),
            create_request(2, &[(ResourceType::Wood, 50)]),
        ];

        // Act
        let result = AllocationEngine::execute(&mut inventory, requests);

        // Assert
        assert_eq!(
            *result.get(&1).unwrap().get(&ResourceType::Wood).unwrap(),
            75,
            "150 * 0.5 = 75"
        );
        assert_eq!(
            *result.get(&2).unwrap().get(&ResourceType::Wood).unwrap(),
            25,
            "50 * 0.5 = 25"
        );

        // Inventory is completely empty
        assert_eq!(*inventory.resources.get(&ResourceType::Wood).unwrap(), 0);
    }

    #[test]
    fn test_execute_missing_resource() {
        // Arrange
        let mut inventory = ResourceInventory::new(1000);
        inventory.add(ResourceType::Wood, 50);
        // Stone is missing entirely from the inventory

        let requests = vec![create_request(
            1,
            &[(ResourceType::Wood, 20), (ResourceType::Stone, 50)],
        )];

        // Act
        let result = AllocationEngine::execute(&mut inventory, requests);

        // Assert
        // Wood is allocated normally
        assert_eq!(
            *result.get(&1).unwrap().get(&ResourceType::Wood).unwrap(),
            20
        );

        // Stone was completely ignored and not added to the result
        assert!(result.get(&1).unwrap().get(&ResourceType::Stone).is_none());
    }

    #[test]
    fn test_execute_labour_is_allocated_but_not_consumed() {
        // Arrange
        let mut inventory = ResourceInventory::new(1000);
        inventory.add(ResourceType::Wood, 50);
        inventory.add(ResourceType::Labour, 100); // We have 100 available workers

        let requests = vec![
            create_request(1, &[(ResourceType::Wood, 10), (ResourceType::Labour, 40)]),
            create_request(2, &[(ResourceType::Labour, 80)]), // Total labour demand: 120
        ];

        // Act
        let result = AllocationEngine::execute(&mut inventory, requests);

        // Assert
        // Ratio for Labour = 100 / 120 = ~0.833
        assert_eq!(
            *result.get(&1).unwrap().get(&ResourceType::Labour).unwrap(),
            33,
            "40 * 0.833 rounded down"
        );
        assert_eq!(
            *result.get(&2).unwrap().get(&ResourceType::Labour).unwrap(),
            66,
            "80 * 0.833 rounded down"
        );

        // Wood was consumed as it is a material resource
        assert_eq!(
            *inventory.resources.get(&ResourceType::Wood).unwrap(),
            40,
            "50 - 10 = 40"
        );

        // Labour remains untouched in the inventory (renewable resource)
        assert_eq!(
            *inventory.resources.get(&ResourceType::Labour).unwrap(),
            100
        );
    }

    #[test]
    fn test_execute_empty_requests() {
        // Arrange
        let mut inventory = ResourceInventory::new(1000);
        inventory.add(ResourceType::Wood, 50);

        let requests: Vec<DemandRequest> = vec![];

        // Act
        let result = AllocationEngine::execute(&mut inventory, requests);

        // Assert
        assert!(
            result.is_empty(),
            "Result should be empty when no requests are provided"
        );
        assert_eq!(
            *inventory.resources.get(&ResourceType::Wood).unwrap(),
            50,
            "Inventory should remain untouched"
        );
    }

    #[test]
    fn test_execute_fractional_floor_rounding() {
        // Arrange
        let mut inventory = ResourceInventory::new(1000);
        inventory.add(ResourceType::Wood, 10);

        // Demand is 15, inventory is 10. Ratio: 10 / 15 = 0.666...
        let requests = vec![create_request(1, &[(ResourceType::Wood, 15)])];

        // Act
        let result = AllocationEngine::execute(&mut inventory, requests);

        // Assert
        // 15 * 0.666... = 10. Floating point math can be tricky, this ensures we correctly floor to 10.
        assert_eq!(
            *result.get(&1).unwrap().get(&ResourceType::Wood).unwrap(),
            10
        );
        assert_eq!(
            *inventory.resources.get(&ResourceType::Wood).unwrap(),
            0,
            "Inventory should be fully depleted"
        );
    }
}
