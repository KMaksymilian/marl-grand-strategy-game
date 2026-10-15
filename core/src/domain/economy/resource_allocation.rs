use crate::domain::economy::resource::{ResourceInventory, ResourceType};
use crate::domain::ids::{BuildingId, ProjectId};
use crate::domain::trading::trade_log::TradeLog;
use crate::domain::trading::trade_log_demand::TradeLogDemand;
use crate::domain::trading::trade_offer::TradeOffer;
use std::collections::{BTreeMap, HashMap};

/// Distinct allocation namespaces, local to one settlement.
/// Ordering is used only to break equal-remainder ties deterministically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AllocationRecipient {
    Population,
    Building(BuildingId),
    Project(ProjectId),
}

#[derive(Debug, Clone)]
pub struct DemandRequest {
    pub recipient: AllocationRecipient,
    pub demand: HashMap<ResourceType, u32>,
}

pub type AllocationResult = HashMap<AllocationRecipient, HashMap<ResourceType, u32>>;

pub struct AllocationEngine;

impl AllocationEngine {
    pub fn execute_trading_log(inventory: &mut ResourceInventory, demands: Vec<TradeLogDemand>) -> TradeLog {
        let mut trade_log: TradeLog = TradeLog::new();
        for demand in demands {
            let resource_type: ResourceType = demand.resource_type;
            let mut trade_offer: TradeOffer = demand.trade_offer;
            if !inventory.consume(resource_type, trade_offer.amount_to_sell) {
                trade_offer.amount_to_sell = inventory.amount(resource_type);
                inventory.consume(resource_type, inventory.amount(resource_type));
            }
            trade_log.add_or_update_trade_offer(resource_type, trade_offer.amount_to_sell, trade_offer.min_price_per_unit);
        }
        trade_log
    }

    pub fn execute(
        inventory: &mut ResourceInventory,
        requests: Vec<DemandRequest>,
    ) -> AllocationResult {
        // Combine repeated requests (e.g. population tiers) before rounding.
        let mut demands: BTreeMap<ResourceType, BTreeMap<AllocationRecipient, u128>> =
            BTreeMap::new();
        for request in requests {
            for (resource, amount) in request.demand {
                if amount > 0 {
                    *demands
                        .entry(resource)
                        .or_default()
                        .entry(request.recipient)
                        .or_default() += u128::from(amount);
                }
            }
        }
        let mut final_allocations: AllocationResult = HashMap::new();
        for (resource, recipients) in demands {
            let available = inventory.resources().get(&resource).copied().unwrap_or(0);
            let total: u128 = recipients.values().sum();
            let budget = u128::from(available).min(total);
            if budget == 0 {
                continue;
            }

            let mut shares: Vec<_> = recipients
                .into_iter()
                .map(|(recipient, needed)| {
                    let numerator = needed * budget;
                    (recipient, numerator / total, numerator % total)
                })
                .collect();
            let distributed: u128 = shares.iter().map(|(_, amount, _)| amount).sum();
            let remaining = budget - distributed;
            shares.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
            for share in shares.iter_mut().take(remaining as usize) {
                share.1 += 1;
            }

            // Commit the material withdrawal before publishing any grants.
            // Labour is a per-turn capacity: shared, but not physically consumed.
            let budget = u32::try_from(budget).expect("budget cannot exceed inventory");
            if resource != ResourceType::Labour && !inventory.consume(resource, budget) {
                continue;
            }
            for (recipient, amount, _) in shares {
                if amount > 0 {
                    final_allocations.entry(recipient).or_default().insert(
                        resource,
                        u32::try_from(amount).expect("share cannot exceed budget"),
                    );
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

    #[test]
    fn allocations_conserve_stock_and_ignore_request_order() {
        let recipients = [
            AllocationRecipient::Population,
            AllocationRecipient::Building(BuildingId(0)),
            AllocationRecipient::Project(ProjectId(0)),
        ];
        for resource in [ResourceType::Wood, ResourceType::Labour] {
            for available in 0..=8_u32 {
                for first in 0..=4_u32 {
                    for second in 0..=4_u32 {
                        for third in 0..=4_u32 {
                            let demands = [first, second, third];
                            let requests: Vec<_> = recipients
                                .iter()
                                .zip(demands)
                                .map(|(&recipient, amount)| DemandRequest {
                                    recipient,
                                    demand: HashMap::from([(resource, amount)]),
                                })
                                .collect();
                            let mut inventory = ResourceInventory::new(100);
                            inventory.add(resource, available);
                            let result =
                                AllocationEngine::execute(&mut inventory, requests.clone());
                            let mut total_allocated = 0;
                            for (recipient, demand) in recipients.iter().zip(demands) {
                                let allocated = result
                                    .get(recipient)
                                    .and_then(|grant| grant.get(&resource))
                                    .copied()
                                    .unwrap_or(0);
                                assert!(allocated <= demand);
                                total_allocated += allocated;
                            }
                            assert_eq!(total_allocated, available.min(first + second + third));
                            assert_eq!(
                                inventory.amount(resource),
                                if resource == ResourceType::Labour {
                                    available
                                } else {
                                    available - total_allocated
                                }
                            );

                            let mut reversed_inventory = ResourceInventory::new(100);
                            reversed_inventory.add(resource, available);
                            let reversed = AllocationEngine::execute(
                                &mut reversed_inventory,
                                requests.into_iter().rev().collect(),
                            );
                            assert_eq!(result, reversed);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn demand_totals_can_exceed_u32_without_overflow() {
        let mut inventory = ResourceInventory::new(u32::MAX);
        inventory.add(ResourceType::Wood, u32::MAX);
        let result = AllocationEngine::execute(
            &mut inventory,
            vec![
                create_request(1, &[(ResourceType::Wood, u32::MAX)]),
                create_request(2, &[(ResourceType::Wood, u32::MAX)]),
            ],
        );
        assert_eq!(
            result[&AllocationRecipient::Building(BuildingId(1))][&ResourceType::Wood],
            2_147_483_648
        );
        assert_eq!(
            result[&AllocationRecipient::Building(BuildingId(2))][&ResourceType::Wood],
            2_147_483_647
        );
        assert_eq!(inventory.resources()[&ResourceType::Wood], 0);
    }

    #[test]
    fn repeated_requests_are_combined_before_distributing_remainders() {
        let mut inventory = ResourceInventory::new(100);
        inventory.add(ResourceType::Wood, 2);
        let result = AllocationEngine::execute(
            &mut inventory,
            vec![
                create_request(1, &[(ResourceType::Wood, 1)]),
                create_request(1, &[(ResourceType::Wood, 1)]),
                create_request(2, &[(ResourceType::Wood, 2)]),
            ],
        );
        assert_eq!(
            result[&AllocationRecipient::Building(BuildingId(1))][&ResourceType::Wood],
            1
        );
        assert_eq!(
            result[&AllocationRecipient::Building(BuildingId(2))][&ResourceType::Wood],
            1
        );
        assert_eq!(inventory.resources()[&ResourceType::Wood], 0);
    }

    #[test]
    fn one_recipient_can_accumulate_demand_above_u32() {
        let mut inventory = ResourceInventory::new(u32::MAX);
        inventory.add(ResourceType::Wood, u32::MAX);
        let result = AllocationEngine::execute(
            &mut inventory,
            vec![
                create_request(1, &[(ResourceType::Wood, u32::MAX)]),
                create_request(1, &[(ResourceType::Wood, u32::MAX)]),
            ],
        );
        assert_eq!(
            result[&AllocationRecipient::Building(BuildingId(1))][&ResourceType::Wood],
            u32::MAX
        );
        assert_eq!(inventory.resources()[&ResourceType::Wood], 0);
    }

    // Helper method to quickly build requests
    fn create_request(entity_id: u32, demands: &[(ResourceType, u32)]) -> DemandRequest {
        let mut demand = HashMap::new();
        for &(res, amount) in demands {
            demand.insert(res, amount);
        }
        DemandRequest {
            recipient: AllocationRecipient::Building(BuildingId(entity_id)),
            demand,
        }
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
            *result
                .get(&AllocationRecipient::Building(BuildingId(1)))
                .unwrap()
                .get(&ResourceType::Wood)
                .unwrap(),
            40
        );
        assert_eq!(
            *result
                .get(&AllocationRecipient::Building(BuildingId(1)))
                .unwrap()
                .get(&ResourceType::Stone)
                .unwrap(),
            20
        );

        // Entity 2 received exactly what it asked for
        assert_eq!(
            *result
                .get(&AllocationRecipient::Building(BuildingId(2)))
                .unwrap()
                .get(&ResourceType::Wood)
                .unwrap(),
            50
        );

        // Inventory is properly depleted
        assert_eq!(
            *inventory.resources().get(&ResourceType::Wood).unwrap(),
            10,
            "100 - (40 + 50) = 10"
        );
        assert_eq!(
            *inventory.resources().get(&ResourceType::Stone).unwrap(),
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
            *result
                .get(&AllocationRecipient::Building(BuildingId(1)))
                .unwrap()
                .get(&ResourceType::Wood)
                .unwrap(),
            75,
            "150 * 0.5 = 75"
        );
        assert_eq!(
            *result
                .get(&AllocationRecipient::Building(BuildingId(2)))
                .unwrap()
                .get(&ResourceType::Wood)
                .unwrap(),
            25,
            "50 * 0.5 = 25"
        );

        // Inventory is completely empty
        assert_eq!(*inventory.resources().get(&ResourceType::Wood).unwrap(), 0);
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
            *result
                .get(&AllocationRecipient::Building(BuildingId(1)))
                .unwrap()
                .get(&ResourceType::Wood)
                .unwrap(),
            20
        );

        // Stone was completely ignored and not added to the result
        assert!(
            result
                .get(&AllocationRecipient::Building(BuildingId(1)))
                .unwrap()
                .get(&ResourceType::Stone)
                .is_none()
        );
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
            *result
                .get(&AllocationRecipient::Building(BuildingId(1)))
                .unwrap()
                .get(&ResourceType::Labour)
                .unwrap(),
            33,
            "40 * 0.833 rounded down"
        );
        assert_eq!(
            *result
                .get(&AllocationRecipient::Building(BuildingId(2)))
                .unwrap()
                .get(&ResourceType::Labour)
                .unwrap(),
            67,
            "Largest remainder receives the final worker"
        );

        // Wood was consumed as it is a material resource
        assert_eq!(
            *inventory.resources().get(&ResourceType::Wood).unwrap(),
            40,
            "50 - 10 = 40"
        );

        // Labour remains untouched in the inventory (renewable resource)
        assert_eq!(
            *inventory.resources().get(&ResourceType::Labour).unwrap(),
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
            *inventory.resources().get(&ResourceType::Wood).unwrap(),
            50,
            "Inventory should remain untouched"
        );
    }

    #[test]
    fn test_execute_exact_fractional_allocation() {
        // Arrange
        let mut inventory = ResourceInventory::new(1000);
        inventory.add(ResourceType::Wood, 10);

        // Demand is 15, inventory is 10. Ratio: 10 / 15 = 0.666...
        let requests = vec![create_request(1, &[(ResourceType::Wood, 15)])];

        // Act
        let result = AllocationEngine::execute(&mut inventory, requests);

        // Assert
        // A single recipient receives the entire available amount exactly.
        assert_eq!(
            *result
                .get(&AllocationRecipient::Building(BuildingId(1)))
                .unwrap()
                .get(&ResourceType::Wood)
                .unwrap(),
            10
        );
        assert_eq!(
            *inventory.resources().get(&ResourceType::Wood).unwrap(),
            0,
            "Inventory should be fully depleted"
        );
    }
}
