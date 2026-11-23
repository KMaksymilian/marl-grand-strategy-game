use crate::domain::economy::storage_order::StorageOrderError;
use crate::domain::economy::{
    CapacityError, InventoryRejection, ResourceCatalog, ResourceInventory, ResourceType,
};
use crate::domain::settlement::turn_report::{InventoryLoss, InventoryLossStage};
use std::collections::HashMap;

#[derive(Debug)]
pub struct SettlementInventoryManager {
    pub(in crate::domain::settlement) main: ResourceInventory,
    pub(in crate::domain::settlement) pending_production: ResourceInventory,
    base_capacity: u32,
}

impl SettlementInventoryManager {
    pub fn new(capacity: u32, resources: &ResourceCatalog) -> Self {
        Self {
            main: ResourceInventory::with_resource_catalog(capacity, resources),
            pending_production: ResourceInventory::with_resource_catalog(capacity, resources),
            base_capacity: capacity,
        }
    }

    pub(in crate::domain::settlement) fn apply_building_capacity(&mut self, bonus: u32) {
        let effective = self.base_capacity.saturating_add(bonus);
        self.main.resize(effective);
        self.pending_production.resize(effective);
    }

    pub(in crate::domain::settlement) fn set_base_capacity(
        &mut self,
        base: u32,
        bonus: u32,
    ) -> Result<(), CapacityError> {
        let effective = base.saturating_add(bonus);
        let stored = self
            .main
            .total_amount()
            .max(self.pending_production.total_amount());
        if stored > u64::from(effective) {
            return Err(CapacityError {
                requested: effective,
                stored,
            });
        }
        self.base_capacity = base;
        self.apply_building_capacity(bonus);
        Ok(())
    }

    pub fn set_storage_order(
        &mut self,
        materials: impl AsRef<[ResourceType]>,
    ) -> Result<(), StorageOrderError> {
        let materials = materials.as_ref();
        self.validate_storage_order(materials)?;
        self.main.set_storage_order(materials)?;
        self.pending_production.set_storage_order(materials)
    }

    pub fn storage_order(&self) -> &[ResourceType] {
        self.main.storage_order()
    }

    pub fn validate_storage_order(
        &self,
        materials: &[ResourceType],
    ) -> Result<(), StorageOrderError> {
        self.main.validate_storage_order(materials)?;
        self.pending_production.validate_storage_order(materials)
    }

    /// Reset Labour amount at the start of turn
    pub fn refresh_labor_capacity(&mut self, population: u32) {
        self.main.reset_labour(population);
    }

    /// Restores reservations made from stock retained after a capacity reduction.
    pub(in crate::domain::settlement) fn restore_reserved_stock(
        &mut self,
        batch: HashMap<ResourceType, u64>,
        opening_stock_limit: u32,
    ) -> Vec<InventoryLoss> {
        let capacity = self.main.capacity();
        self.main.resize(opening_stock_limit.max(capacity));
        let rejected = self.main.add_batch(batch);
        self.main.resize(capacity);
        Self::report_rejections(rejected, InventoryLossStage::Returns)
    }

    /// Produced goods are stored in buffer
    pub fn process_production_results(
        &mut self,
        produced: HashMap<ResourceType, u64>,
        returned: HashMap<ResourceType, u64>,
    ) -> Vec<InventoryLoss> {
        let mut losses =
            Self::report_rejections(self.main.add_batch(returned), InventoryLossStage::Returns);
        losses.extend(Self::report_rejections(
            self.pending_production.add_batch(produced),
            InventoryLossStage::ProductionBuffer,
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
        Self::report_rejections(self.main.add_batch(pending), InventoryLossStage::Warehouse)
    }

    fn report_rejections(
        rejections: Vec<InventoryRejection>,
        stage: InventoryLossStage,
    ) -> Vec<InventoryLoss> {
        rejections
            .into_iter()
            .map(|rejection| InventoryLoss {
                resource: rejection.resource,
                amount: rejection.amount,
                stage,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::ResourceType;
    use crate::test_support::material;
    use std::collections::HashMap;

    #[test]
    fn configured_priority_controls_buffer_overflow() {
        let mut manager = SettlementInventoryManager::new(3, ResourceCatalog::standard());
        manager
            .set_storage_order(
                [
                    &[material("grain"), material("wood"), material("stone")],
                    &ResourceCatalog::standard().materials()[3..],
                ]
                .concat(),
            )
            .unwrap();
        let losses = manager.process_production_results(
            HashMap::from([(material("wood"), 3), (material("grain"), 3)]),
            HashMap::new(),
        );
        assert_eq!(
            manager.pending_production.resources(),
            &HashMap::from([(material("grain"), 3)])
        );
        assert_eq!(
            losses,
            vec![InventoryLoss {
                resource: material("wood"),
                amount: 3,
                stage: InventoryLossStage::ProductionBuffer
            }]
        );
    }

    #[test]
    fn configured_priority_controls_return_overflow() {
        let mut manager = SettlementInventoryManager::new(3, ResourceCatalog::standard());
        manager
            .set_storage_order(
                [
                    &[material("grain"), material("wood"), material("stone")],
                    &ResourceCatalog::standard().materials()[3..],
                ]
                .concat(),
            )
            .unwrap();
        let losses = manager.process_production_results(
            HashMap::new(),
            HashMap::from([(material("wood"), 3), (material("grain"), 3)]),
        );
        assert_eq!(
            manager.main.resources(),
            &HashMap::from([(material("grain"), 3)])
        );
        assert_eq!(
            losses,
            vec![InventoryLoss {
                resource: material("wood"),
                amount: 3,
                stage: InventoryLossStage::Returns
            }]
        );
    }

    #[test]
    fn configured_priority_controls_warehouse_transfer() {
        let mut manager = SettlementInventoryManager::new(6, ResourceCatalog::standard());
        manager.main.set_capacity(3).unwrap();
        manager
            .set_storage_order(
                [
                    &[material("grain"), material("wood"), material("stone")],
                    &ResourceCatalog::standard().materials()[3..],
                ]
                .concat(),
            )
            .unwrap();
        assert!(
            manager
                .process_production_results(
                    HashMap::from([(material("wood"), 3), (material("grain"), 3)]),
                    HashMap::new(),
                )
                .is_empty()
        );
        assert_eq!(
            manager.commit_pending_production(),
            vec![InventoryLoss {
                resource: material("wood"),
                amount: 3,
                stage: InventoryLossStage::Warehouse,
            }]
        );
        assert_eq!(
            manager.main.resources(),
            &HashMap::from([(material("grain"), 3)])
        );
    }

    #[test]
    fn equal_priorities_use_stable_resource_order_for_all_insertions() {
        for order in [
            [material("wood"), material("stone"), material("grain")],
            [material("wood"), material("grain"), material("stone")],
            [material("stone"), material("wood"), material("grain")],
            [material("stone"), material("grain"), material("wood")],
            [material("grain"), material("wood"), material("stone")],
            [material("grain"), material("stone"), material("wood")],
        ] {
            let mut manager = SettlementInventoryManager::new(4, ResourceCatalog::standard());
            manager
                .set_storage_order(
                    [
                        &[material("stone"), material("grain"), material("wood")],
                        &ResourceCatalog::standard().materials()[3..],
                    ]
                    .concat(),
                )
                .unwrap();
            let losses = manager.process_production_results(
                order
                    .into_iter()
                    .map(|resource| (resource, 3_u64))
                    .collect(),
                HashMap::new(),
            );
            assert_eq!(
                manager.pending_production.resources(),
                &HashMap::from([(material("stone"), 3), (material("grain"), 1)])
            );
            assert_eq!(
                losses,
                vec![
                    InventoryLoss {
                        resource: material("grain"),
                        amount: 2,
                        stage: InventoryLossStage::ProductionBuffer
                    },
                    InventoryLoss {
                        resource: material("wood"),
                        amount: 3,
                        stage: InventoryLossStage::ProductionBuffer
                    },
                ]
            );
        }
    }

    #[test]
    fn product_priority_does_not_override_returns_phase() {
        let mut manager = SettlementInventoryManager::new(1, ResourceCatalog::standard());
        manager
            .set_storage_order(
                [
                    &[material("grain"), material("wood"), material("stone")],
                    &ResourceCatalog::standard().materials()[3..],
                ]
                .concat(),
            )
            .unwrap();
        assert!(
            manager
                .process_production_results(
                    HashMap::from([(material("grain"), 1)]),
                    HashMap::from([(material("wood"), 1)]),
                )
                .is_empty()
        );
        assert_eq!(
            manager.commit_pending_production(),
            vec![InventoryLoss {
                resource: material("grain"),
                amount: 1,
                stage: InventoryLossStage::Warehouse,
            }]
        );
        assert_eq!(
            manager.main.resources(),
            &HashMap::from([(material("wood"), 1)])
        );
    }

    #[test]
    fn overflow_order_is_stable_for_all_resource_insertion_orders() {
        let orders = [
            [material("wood"), material("stone"), material("grain")],
            [material("wood"), material("grain"), material("stone")],
            [material("stone"), material("wood"), material("grain")],
            [material("stone"), material("grain"), material("wood")],
            [material("grain"), material("wood"), material("stone")],
            [material("grain"), material("stone"), material("wood")],
        ];
        for order in orders {
            let mut manager = SettlementInventoryManager::new(2, ResourceCatalog::standard());
            let produced = order
                .into_iter()
                .map(|resource| (resource, 3_u64))
                .collect();
            let losses = manager.process_production_results(produced, HashMap::new());
            assert_eq!(
                manager.pending_production.resources(),
                &HashMap::from([(material("wood"), 2)])
            );
            assert_eq!(
                losses,
                vec![
                    InventoryLoss {
                        resource: material("wood"),
                        amount: 1,
                        stage: InventoryLossStage::ProductionBuffer
                    },
                    InventoryLoss {
                        resource: material("stone"),
                        amount: 3,
                        stage: InventoryLossStage::ProductionBuffer
                    },
                    InventoryLoss {
                        resource: material("grain"),
                        amount: 3,
                        stage: InventoryLossStage::ProductionBuffer
                    },
                ]
            );
            assert!(manager.commit_pending_production().is_empty());
            assert_eq!(
                manager.main.resources(),
                &HashMap::from([(material("wood"), 2)])
            );
        }
    }

    #[test]
    fn returns_are_retained_before_new_products() {
        let mut manager = SettlementInventoryManager::new(2, ResourceCatalog::standard());
        manager.main.add(material("stone"), 1);
        assert!(
            manager
                .process_production_results(
                    HashMap::from([(material("wood"), 2)]),
                    HashMap::from([(material("grain"), 1)]),
                )
                .is_empty()
        );
        assert_eq!(
            manager.commit_pending_production(),
            vec![InventoryLoss {
                resource: material("wood"),
                amount: 2,
                stage: InventoryLossStage::Warehouse,
            }]
        );
        assert_eq!(manager.main.resources()[&material("grain")], 1);
        assert!(!manager.main.resources().contains_key(&material("wood")));
    }

    #[test]
    fn returned_overflow_is_reported_separately() {
        let mut manager = SettlementInventoryManager::new(1, ResourceCatalog::standard());
        manager.main.add(material("wood"), 1);
        assert_eq!(
            manager.process_production_results(
                HashMap::new(),
                HashMap::from([(material("stone"), 2)])
            ),
            vec![InventoryLoss {
                resource: material("stone"),
                amount: 2,
                stage: InventoryLossStage::Returns
            }]
        );
    }

    #[test]
    fn test_new_initializes_with_correct_capacities() {
        // Arrange & Act
        let manager = SettlementInventoryManager::new(150, ResourceCatalog::standard());

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
        let mut manager = SettlementInventoryManager::new(100, ResourceCatalog::standard());

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
        let mut manager = SettlementInventoryManager::new(100, ResourceCatalog::standard());

        let mut produced = HashMap::new();
        produced.insert(material("wood"), 30);

        let mut returned = HashMap::new();
        returned.insert(material("stone"), 20);

        // Act
        manager.process_production_results(produced, returned);

        // Assert
        // Returned items go straight back to main
        assert_eq!(manager.main.total_amount(), 20);
        assert_eq!(
            *manager.main.resources().get(&material("stone")).unwrap(),
            20
        );

        // Produced items go to pending buffer
        assert_eq!(manager.pending_production.total_amount(), 30);
        assert_eq!(
            *manager
                .pending_production
                .resources()
                .get(&material("wood"))
                .unwrap(),
            30
        );
    }

    #[test]
    fn test_commit_pending_production_moves_resources_to_main() {
        // Arrange
        let mut manager = SettlementInventoryManager::new(100, ResourceCatalog::standard());

        // Add some initial resources to main
        manager.main.add(material("stone"), 10);

        // Put resources in pending
        let mut produced = HashMap::new();
        produced.insert(material("wood"), 40);
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
            *manager.main.resources().get(&material("wood")).unwrap(),
            40
        );
        assert_eq!(
            *manager.main.resources().get(&material("stone")).unwrap(),
            10
        );
    }

    #[test]
    fn test_commit_pending_production_respects_main_capacity() {
        // Arrange
        let mut manager = SettlementInventoryManager::new(50, ResourceCatalog::standard());

        // Fill main inventory almost to the brim (40/50)
        manager.main.add(material("stone"), 40);

        // Put 30 wood in pending
        let mut produced = HashMap::new();
        produced.insert(material("wood"), 30);
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
            *manager.main.resources().get(&material("wood")).unwrap(),
            10,
            "Only the amount that fits should be transferred from pending"
        );
    }
}
