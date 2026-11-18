use super::ResourceCatalog;
use super::resources::ResourceType;
use super::storage_order::{StorageOrder, StorageOrderError};
use std::collections::HashMap;

/// Resource counters can only be changed through capacity-aware commands.
/// ```compile_fail
/// use simulation_core::economy::{ResourceInventory, ResourceType};
/// let mut inventory = ResourceInventory::new(1);
/// let wood = simulation_core::economy::ResourceCatalog::standard().resolve("wood").unwrap();
/// inventory.resources.insert(wood, 100);
/// ```
#[derive(Debug, Default)]
pub struct ResourceInventory {
    resources: HashMap<ResourceType, u32>,
    max_capacity: u32,
    priorities: StorageOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryAddResult {
    pub accepted: u32,
    pub rejected: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryRejection {
    pub resource: ResourceType,
    pub amount: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityError {
    pub requested: u32,
    pub stored: u64,
}

impl ResourceInventory {
    pub fn resources(&self) -> &HashMap<ResourceType, u32> {
        &self.resources
    }
    pub fn amount(&self, resource: ResourceType) -> u32 {
        self.resources.get(&resource).copied().unwrap_or(0)
    }
    pub(crate) fn reset_labour(&mut self, population: u32) {
        self.resources.insert(ResourceType::Labour, population);
    }
    pub(crate) fn take_all(&mut self) -> HashMap<ResourceType, u32> {
        std::mem::take(&mut self.resources)
    }

    pub fn new(max_capacity: u32) -> Self {
        Self::with_resource_catalog(max_capacity, ResourceCatalog::standard())
    }

    pub fn with_resource_catalog(max_capacity: u32, resources: &ResourceCatalog) -> Self {
        Self {
            resources: HashMap::new(),
            max_capacity,
            priorities: StorageOrder::for_catalog(resources),
        }
    }

    pub fn storage_order(&self) -> &[ResourceType] {
        self.priorities.get()
    }

    /// Change future batch admission priorities without moving existing stock.
    pub fn set_storage_order(
        &mut self,
        materials: impl AsRef<[ResourceType]>,
    ) -> Result<(), StorageOrderError> {
        self.priorities.set(materials)
    }

    pub fn validate_storage_order(
        &self,
        materials: &[ResourceType],
    ) -> Result<(), StorageOrderError> {
        self.priorities.validate_order(materials)
    }

    pub fn knows_material(&self, resource: &ResourceType) -> bool {
        self.storage_order().contains(resource)
    }

    /// Admit a batch in warehouse priority order, returning all rejected quantities.
    pub fn add_batch(&mut self, batch: HashMap<ResourceType, u64>) -> Vec<InventoryRejection> {
        let mut entries: Vec<_> = batch.into_iter().collect();
        entries.sort_unstable_by(|a, b| {
            self.priorities
                .compare(&a.0, &b.0)
                .then_with(|| a.0.cmp(&b.0))
        });
        let mut rejections = Vec::new();
        for (resource, amount) in entries {
            let addition = self.add(resource.clone(), amount.min(u64::from(u32::MAX)) as u32);
            let rejected = amount - u64::from(addition.accepted);
            if rejected > 0 {
                rejections.push(InventoryRejection {
                    resource,
                    amount: rejected,
                });
            }
        }
        rejections
    }
    pub fn capacity(&self) -> u32 {
        self.max_capacity
    }

    /// Sets a new capacity, also below the stored amount. Nothing is discarded:
    /// stock above the capacity stays, and additions wait until it drops below.
    pub(crate) fn resize(&mut self, capacity: u32) {
        self.max_capacity = capacity;
    }

    /// Reject shrinking below stored materials; never silently discard stock.
    pub fn set_capacity(&mut self, capacity: u32) -> Result<(), CapacityError> {
        let stored = self.total_amount();
        if stored > u64::from(capacity) {
            return Err(CapacityError {
                requested: capacity,
                stored,
            });
        }
        self.max_capacity = capacity;
        Ok(())
    }

    pub fn total_amount(&self) -> u64 {
        self.resources
            .iter()
            .filter(|(res, _)| **res != ResourceType::Labour)
            .map(|(_, &amount)| u64::from(amount))
            .sum()
    }
    pub fn add(&mut self, res_type: ResourceType, amount: u32) -> InventoryAddResult {
        if res_type != ResourceType::Labour && !self.knows_material(&res_type) {
            return InventoryAddResult {
                accepted: 0,
                rejected: amount,
            };
        }
        let counter_space = u32::MAX - self.resources.get(&res_type).copied().unwrap_or(0);
        let available_space = if res_type == ResourceType::Labour {
            counter_space
        } else {
            u64::from(self.max_capacity)
                .saturating_sub(self.total_amount())
                .min(u64::from(counter_space)) as u32
        };
        let accepted = amount.min(available_space);
        if accepted > 0 {
            *self.resources.entry(res_type).or_insert(0) += accepted;
        }
        InventoryAddResult {
            accepted,
            rejected: amount - accepted,
        }
    }

    pub fn consume(&mut self, res_type: ResourceType, amount: u32) -> bool {
        if let Some(current) = self.resources.get_mut(&res_type)
            && *current >= amount
        {
            *current -= amount;
            return true;
        }
        false
    }

    pub fn consume_available(&mut self, res_type: ResourceType) -> u32 {
        if let Some(available) = self.resources.get_mut(&res_type) {
            std::mem::take(available)
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::material;

    #[test]
    fn additions_report_both_accepted_and_rejected_units() {
        let mut inventory = ResourceInventory::new(5);
        assert_eq!(
            inventory.add(material("wood"), 8),
            InventoryAddResult {
                accepted: 5,
                rejected: 3
            }
        );
        assert_eq!(
            inventory.add(material("grain"), 2),
            InventoryAddResult {
                accepted: 0,
                rejected: 2
            }
        );
        assert_eq!(inventory.total_amount(), 5);
    }

    #[test]
    fn shrinking_capacity_below_stock_is_rejected_without_changing_data() {
        let mut inventory = ResourceInventory::new(10);
        inventory.add(material("wood"), 6);
        assert_eq!(
            inventory.set_capacity(5),
            Err(CapacityError {
                requested: 5,
                stored: 6
            })
        );
        assert_eq!(inventory.capacity(), 10);
        assert_eq!(inventory.resources[&material("wood")], 6);
        inventory.set_capacity(6).unwrap();
        assert_eq!(inventory.add(material("stone"), 1).rejected, 1);
        inventory.set_capacity(7).unwrap();
        assert_eq!(inventory.add(material("stone"), 1).accepted, 1);
    }

    #[test]
    fn labour_only_obeys_its_counter_limit() {
        let mut inventory = ResourceInventory::new(0);
        assert_eq!(
            inventory.add(ResourceType::Labour, u32::MAX).accepted,
            u32::MAX
        );
        assert_eq!(
            inventory.add(ResourceType::Labour, 1),
            InventoryAddResult {
                accepted: 0,
                rejected: 1
            }
        );
        assert_eq!(inventory.total_amount(), 0);
        inventory.set_capacity(0).unwrap();
    }

    #[test]
    fn invalid_public_stock_above_u32_is_summed_without_overflow() {
        let mut inventory = ResourceInventory::new(u32::MAX);
        inventory.resources.insert(material("wood"), u32::MAX);
        inventory.resources.insert(material("stone"), u32::MAX);
        assert_eq!(inventory.total_amount(), 2 * u64::from(u32::MAX));
        assert_eq!(inventory.add(material("grain"), 1).rejected, 1);
    }

    #[test]
    fn labour_can_be_added_to_a_full_warehouse() {
        let mut inventory = ResourceInventory::new(1);
        inventory.add(material("wood"), 1);
        inventory.add(ResourceType::Labour, 10);
        assert_eq!(inventory.resources.get(&ResourceType::Labour), Some(&10));
        assert_eq!(inventory.total_amount(), 1);
    }

    #[test]
    fn an_overfull_public_inventory_rejects_additions_without_underflow() {
        let mut inventory = ResourceInventory::new(1);
        inventory.resources.insert(material("wood"), 2);
        inventory.add(material("grain"), 1);
        assert_eq!(inventory.total_amount(), 2);
        assert!(!inventory.resources.contains_key(&material("grain")));
    }

    #[test]
    fn test_new_inventory() {
        let inv = ResourceInventory::new(100);
        assert_eq!(inv.max_capacity, 100);
        assert_eq!(inv.total_amount(), 0);
        assert!(inv.resources.is_empty());
    }

    #[test]
    fn test_add_within_capacity() {
        let mut inv = ResourceInventory::new(100);
        inv.add(material("wood"), 40);

        assert_eq!(inv.total_amount(), 40);
        assert_eq!(*inv.resources.get(&material("wood")).unwrap(), 40);
    }

    #[test]
    fn test_add_exceeding_capacity() {
        let mut inv = ResourceInventory::new(50);
        inv.add(material("stone"), 60);

        assert_eq!(inv.total_amount(), 50);
        assert_eq!(*inv.resources.get(&material("stone")).unwrap(), 50);
    }

    #[test]
    fn test_add_multiple_resources_with_overflow() {
        let mut inv = ResourceInventory::new(100);
        inv.add(material("wood"), 30);
        inv.add(material("grain"), 50);

        assert_eq!(inv.total_amount(), 80);

        inv.add(material("stone"), 30);

        assert_eq!(inv.total_amount(), 100);
        assert_eq!(*inv.resources.get(&material("stone")).unwrap(), 20);
    }

    #[test]
    fn test_add_when_already_full() {
        let mut inv = ResourceInventory::new(100);
        inv.add(material("wood"), 100);

        inv.add(material("grain"), 10);

        assert_eq!(inv.total_amount(), 100);
        assert_eq!(inv.resources.get(&material("grain")), None);
    }

    #[test]
    fn test_consume_success() {
        let mut inv = ResourceInventory::new(100);
        inv.add(material("grain"), 50);

        let result = inv.consume(material("grain"), 20);

        assert!(result);
        assert_eq!(inv.total_amount(), 30);
        assert_eq!(*inv.resources.get(&material("grain")).unwrap(), 30);
    }

    #[test]
    fn test_consume_insufficient_amount() {
        let mut inv = ResourceInventory::new(100);
        inv.add(material("wood"), 20);

        let result = inv.consume(material("wood"), 30);

        assert!(!result);
        assert_eq!(inv.total_amount(), 20);
    }

    #[test]
    fn test_consume_nonexistent_resource() {
        let mut inv = ResourceInventory::new(100);

        let result = inv.consume(material("stone"), 10);

        assert!(!result);
    }
}
