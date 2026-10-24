//! Warehouse stock, capacity and material admission order.
use super::super::Settlement;
use crate::economy::ResourceType;
use crate::economy::storage_order::StorageOrderError;

impl Settlement {
    pub fn deposit(
        &mut self,
        resource: ResourceType,
        amount: u32,
    ) -> crate::economy::InventoryAddResult {
        self.inventory_manager.main.add(resource, amount)
    }

    pub fn withdraw(&mut self, resource: ResourceType, amount: u32) -> bool {
        self.inventory_manager.main.consume(resource, amount)
    }

    pub fn set_inventory_capacity(
        &mut self,
        capacity: u32,
    ) -> Result<(), crate::economy::CapacityError> {
        self.inventory_manager.main.set_capacity(capacity)
    }

    /// Applies to incoming storage batches; does not evict existing stock.
    pub fn set_storage_order(
        &mut self,
        materials: impl AsRef<[ResourceType]>,
    ) -> Result<(), StorageOrderError> {
        self.inventory_manager.set_storage_order(materials)
    }
}
