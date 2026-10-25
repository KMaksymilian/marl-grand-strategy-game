use crate::economy::storage_order::{StorageOrder, StorageOrderError};
use crate::economy::{ResourceInventory, ResourceType};
use crate::settlement::turn_report::{InventoryLoss, InventoryLossStage};
use std::collections::HashMap;

#[derive(Debug)]
pub struct SettlementInventoryManager {
    pub(super) main: ResourceInventory,
    pub(super) pending_production: ResourceInventory,
    priorities: StorageOrder,
}

impl SettlementInventoryManager {
    pub fn new(capacity: u32) -> Self {
        Self {
            main: ResourceInventory::new(capacity),
            pending_production: ResourceInventory::new(capacity),
            priorities: StorageOrder::default(),
        }
    }

    pub fn set_storage_order(
        &mut self,
        materials: impl AsRef<[ResourceType]>,
    ) -> Result<(), StorageOrderError> {
        self.priorities.set(materials)
    }

    pub fn storage_order(&self) -> &[ResourceType] {
        self.priorities.get()
    }

    /// Reset Labour amount at the start of turn
    pub fn refresh_labor_capacity(&mut self, population: u32) {
        self.main.reset_labour(population);
    }

    /// Produced goods are stored in buffer
    pub fn process_production_results(
        &mut self,
        produced: HashMap<ResourceType, u64>,
        returned: HashMap<ResourceType, u64>,
    ) -> Vec<InventoryLoss> {
        let mut losses = Self::add_in_order(
            &mut self.main,
            returned,
            InventoryLossStage::Returns,
            &self.priorities,
        );
        losses.extend(Self::add_in_order(
            &mut self.pending_production,
            produced,
            InventoryLossStage::ProductionBuffer,
            &self.priorities,
        ));
        losses
    }

    /// Move goods from buffer to inventory
    pub fn commit_pending_production(&mut self) -> Vec<InventoryLoss> {
        let pending = self
            .pending_production
            .take_all()
            .into_iter()
            .map(|(resource, amount)| (resource, u64::from(amount)))
            .collect();
        Self::add_in_order(
            &mut self.main,
            pending,
            InventoryLossStage::Warehouse,
            &self.priorities,
        )
    }

    /// Trade goods go straight to the warehouse in storage order.
    pub fn admit_trade_returns(
        &mut self,
        returned: HashMap<ResourceType, u64>,
    ) -> Vec<InventoryLoss> {
        Self::add_in_order(
            &mut self.main,
            returned,
            InventoryLossStage::TradeReturns,
            &self.priorities,
        )
    }

    fn add_in_order(
        inventory: &mut ResourceInventory,
        batch: HashMap<ResourceType, u64>,
        stage: InventoryLossStage,
        priorities: &StorageOrder,
    ) -> Vec<InventoryLoss> {
        let mut entries: Vec<_> = batch.into_iter().collect();
        entries.sort_unstable_by(|a, b| priorities.compare(a.0, b.0));
        let mut losses = Vec::new();
        for (resource, amount) in entries {
            let addition = inventory.add(resource, amount.min(u64::from(u32::MAX)) as u32);
            let rejected = amount - u64::from(addition.accepted);
            if rejected > 0 {
                losses.push(InventoryLoss {
                    resource,
                    amount: rejected,
                    stage,
                });
            }
        }
        losses
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::ResourceType;
    use std::collections::HashMap;

    #[test]
    fn configured_priority_controls_buffer_overflow() {
        let mut manager = SettlementInventoryManager::new(3);
        manager
            .set_storage_order([ResourceType::Grain, ResourceType::Wood, ResourceType::Stone])
            .unwrap();
        let losses = manager.process_production_results(
            HashMap::from([(ResourceType::Wood, 3), (ResourceType::Grain, 3)]),
            HashMap::new(),
        );
        assert_eq!(
            manager.pending_production.resources(),
            &HashMap::from([(ResourceType::Grain, 3)])
        );
        assert_eq!(
            losses,
            vec![InventoryLoss {
                resource: ResourceType::Wood,
                amount: 3,
                stage: InventoryLossStage::ProductionBuffer
            }]
        );
    }

    #[test]
    fn configured_priority_controls_return_overflow() {
        let mut manager = SettlementInventoryManager::new(3);
        manager
            .set_storage_order([ResourceType::Grain, ResourceType::Wood, ResourceType::Stone])
            .unwrap();
        let losses = manager.process_production_results(
            HashMap::new(),
            HashMap::from([(ResourceType::Wood, 3), (ResourceType::Grain, 3)]),
        );
        assert_eq!(
            manager.main.resources(),
            &HashMap::from([(ResourceType::Grain, 3)])
        );
        assert_eq!(
            losses,
            vec![InventoryLoss {
                resource: ResourceType::Wood,
                amount: 3,
                stage: InventoryLossStage::Returns
            }]
        );
    }

    #[test]
    fn configured_priority_controls_warehouse_transfer() {
        let mut manager = SettlementInventoryManager::new(6);
        manager.main.set_capacity(3).unwrap();
        manager
            .set_storage_order([ResourceType::Grain, ResourceType::Wood, ResourceType::Stone])
            .unwrap();
        assert!(
            manager
                .process_production_results(
                    HashMap::from([(ResourceType::Wood, 3), (ResourceType::Grain, 3)]),
                    HashMap::new(),
                )
                .is_empty()
        );
        assert_eq!(
            manager.commit_pending_production(),
            vec![InventoryLoss {
                resource: ResourceType::Wood,
                amount: 3,
                stage: InventoryLossStage::Warehouse,
            }]
        );
        assert_eq!(
            manager.main.resources(),
            &HashMap::from([(ResourceType::Grain, 3)])
        );
    }

    #[test]
    fn equal_priorities_use_stable_resource_order_for_all_insertions() {
        use ResourceType::{Grain, Stone, Wood};
        for order in [
            [Wood, Stone, Grain],
            [Wood, Grain, Stone],
            [Stone, Wood, Grain],
            [Stone, Grain, Wood],
            [Grain, Wood, Stone],
            [Grain, Stone, Wood],
        ] {
            let mut manager = SettlementInventoryManager::new(4);
            manager.set_storage_order([Stone, Grain, Wood]).unwrap();
            let losses = manager.process_production_results(
                order
                    .into_iter()
                    .map(|resource| (resource, 3_u64))
                    .collect(),
                HashMap::new(),
            );
            assert_eq!(
                manager.pending_production.resources(),
                &HashMap::from([(Stone, 3), (Grain, 1)])
            );
            assert_eq!(
                losses,
                vec![
                    InventoryLoss {
                        resource: Grain,
                        amount: 2,
                        stage: InventoryLossStage::ProductionBuffer
                    },
                    InventoryLoss {
                        resource: Wood,
                        amount: 3,
                        stage: InventoryLossStage::ProductionBuffer
                    },
                ]
            );
        }
    }

    #[test]
    fn product_priority_does_not_override_returns_phase() {
        let mut manager = SettlementInventoryManager::new(1);
        manager
            .set_storage_order([ResourceType::Grain, ResourceType::Wood, ResourceType::Stone])
            .unwrap();
        assert!(
            manager
                .process_production_results(
                    HashMap::from([(ResourceType::Grain, 1)]),
                    HashMap::from([(ResourceType::Wood, 1)]),
                )
                .is_empty()
        );
        assert_eq!(
            manager.commit_pending_production(),
            vec![InventoryLoss {
                resource: ResourceType::Grain,
                amount: 1,
                stage: InventoryLossStage::Warehouse,
            }]
        );
        assert_eq!(
            manager.main.resources(),
            &HashMap::from([(ResourceType::Wood, 1)])
        );
    }

    #[test]
    fn overflow_order_is_stable_for_all_resource_insertion_orders() {
        use ResourceType::{Grain, Stone, Wood};
        let orders = [
            [Wood, Stone, Grain],
            [Wood, Grain, Stone],
            [Stone, Wood, Grain],
            [Stone, Grain, Wood],
            [Grain, Wood, Stone],
            [Grain, Stone, Wood],
        ];
        for order in orders {
            let mut manager = SettlementInventoryManager::new(2);
            let produced = order
                .into_iter()
                .map(|resource| (resource, 3_u64))
                .collect();
            let losses = manager.process_production_results(produced, HashMap::new());
            assert_eq!(
                manager.pending_production.resources(),
                &HashMap::from([(Wood, 2)])
            );
            assert_eq!(
                losses,
                vec![
                    InventoryLoss {
                        resource: Wood,
                        amount: 1,
                        stage: InventoryLossStage::ProductionBuffer
                    },
                    InventoryLoss {
                        resource: Stone,
                        amount: 3,
                        stage: InventoryLossStage::ProductionBuffer
                    },
                    InventoryLoss {
                        resource: Grain,
                        amount: 3,
                        stage: InventoryLossStage::ProductionBuffer
                    },
                ]
            );
            assert!(manager.commit_pending_production().is_empty());
            assert_eq!(manager.main.resources(), &HashMap::from([(Wood, 2)]));
        }
    }

    #[test]
    fn returns_are_retained_before_new_products() {
        let mut manager = SettlementInventoryManager::new(2);
        manager.main.add(ResourceType::Stone, 1);
        assert!(
            manager
                .process_production_results(
                    HashMap::from([(ResourceType::Wood, 2)]),
                    HashMap::from([(ResourceType::Grain, 1)]),
                )
                .is_empty()
        );
        assert_eq!(
            manager.commit_pending_production(),
            vec![InventoryLoss {
                resource: ResourceType::Wood,
                amount: 2,
                stage: InventoryLossStage::Warehouse,
            }]
        );
        assert_eq!(manager.main.resources()[&ResourceType::Grain], 1);
        assert!(!manager.main.resources().contains_key(&ResourceType::Wood));
    }

    #[test]
    fn returned_overflow_is_reported_separately() {
        let mut manager = SettlementInventoryManager::new(1);
        manager.main.add(ResourceType::Wood, 1);
        assert_eq!(
            manager.process_production_results(
                HashMap::new(),
                HashMap::from([(ResourceType::Stone, 2)])
            ),
            vec![InventoryLoss {
                resource: ResourceType::Stone,
                amount: 2,
                stage: InventoryLossStage::Returns
            }]
        );
    }

    #[test]
    fn test_new_initializes_with_correct_capacities() {
        // Arrange & Act
        let manager = SettlementInventoryManager::new(150);

        // Assert
        assert_eq!(
            manager.main.capacity(),
            150,
            "Main inventory should have correct capacity"
        );
        assert_eq!(
            manager.pending_production.capacity(),
            150,
            "Pending inventory should have correct capacity"
        );
        assert_eq!(manager.main.total_amount(), 0);
        assert_eq!(manager.pending_production.total_amount(), 0);
    }

    #[test]
    fn test_refresh_labor_capacity_inserts_and_overwrites() {
        // Arrange
        let mut manager = SettlementInventoryManager::new(100);

        // Act 1: Initial set
        manager.refresh_labor_capacity(50);

        // Assert 1
        assert_eq!(
            *manager.main.resources().get(&ResourceType::Labour).unwrap(),
            50,
            "Labour should be set to 50"
        );
        assert_eq!(
            manager.main.total_amount(),
            0,
            "Labour should not count towards total capacity limits"
        );

        // Act 2: Overwrite on next turn
        manager.refresh_labor_capacity(60);

        // Assert 2
        assert_eq!(
            *manager.main.resources().get(&ResourceType::Labour).unwrap(),
            60,
            "Labour should be overwritten to 60, not added"
        );
    }

    #[test]
    fn test_process_production_results_routes_correctly() {
        // Arrange
        let mut manager = SettlementInventoryManager::new(100);

        let mut produced = HashMap::new();
        produced.insert(ResourceType::Wood, 30);

        let mut returned = HashMap::new();
        returned.insert(ResourceType::Stone, 20);

        // Act
        manager.process_production_results(produced, returned);

        // Assert
        // Returned items go straight back to main
        assert_eq!(manager.main.total_amount(), 20);
        assert_eq!(
            *manager.main.resources().get(&ResourceType::Stone).unwrap(),
            20
        );

        // Produced items go to pending buffer
        assert_eq!(manager.pending_production.total_amount(), 30);
        assert_eq!(
            *manager
                .pending_production
                .resources()
                .get(&ResourceType::Wood)
                .unwrap(),
            30
        );
    }

    #[test]
    fn test_commit_pending_production_moves_resources_to_main() {
        // Arrange
        let mut manager = SettlementInventoryManager::new(100);

        // Add some initial resources to main
        manager.main.add(ResourceType::Stone, 10);

        // Put resources in pending
        let mut produced = HashMap::new();
        produced.insert(ResourceType::Wood, 40);
        manager.process_production_results(produced, HashMap::new());

        // Act
        manager.commit_pending_production();

        // Assert
        assert!(
            manager.pending_production.resources().is_empty(),
            "Pending production should be drained completely"
        );

        assert_eq!(
            manager.main.total_amount(),
            50, // 10 stone + 40 wood
            "Main inventory should now contain both existing and committed resources"
        );
        assert_eq!(
            *manager.main.resources().get(&ResourceType::Wood).unwrap(),
            40
        );
        assert_eq!(
            *manager.main.resources().get(&ResourceType::Stone).unwrap(),
            10
        );
    }

    #[test]
    fn test_commit_pending_production_respects_main_capacity() {
        // Arrange
        let mut manager = SettlementInventoryManager::new(50);

        // Fill main inventory almost to the brim (40/50)
        manager.main.add(ResourceType::Stone, 40);

        // Put 30 wood in pending
        let mut produced = HashMap::new();
        produced.insert(ResourceType::Wood, 30);
        manager.process_production_results(produced, HashMap::new());

        // Act
        manager.commit_pending_production();

        // Assert
        assert!(
            manager.pending_production.resources().is_empty(),
            "Pending production is always drained, even if main can't fit everything"
        );

        assert_eq!(
            manager.main.total_amount(),
            50,
            "Main inventory should be capped at its max capacity (50)"
        );

        // Only 10 wood should have made it in (50 max - 40 stone = 10 space left)
        assert_eq!(
            *manager.main.resources().get(&ResourceType::Wood).unwrap(),
            10,
            "Only the amount that fits should be transferred from pending"
        );
    }
}
