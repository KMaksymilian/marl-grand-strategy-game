//! Resource catalogue, stock, currency and allocation rules.
pub mod allocation;
pub mod cost;
pub mod gold_inventory;
pub mod inventory;
mod resource_catalog;
pub mod resources;
pub mod storage_order;

pub use inventory::{CapacityError, InventoryAddResult, InventoryRejection, ResourceInventory};
pub use resource_catalog::{
    FoodClass, ResourceCatalog, ResourceCatalogError, ResourceCategory, ResourceDefinition,
};
pub use resources::{ResourceId, ResourceType};

/// Integer material units represented by one whole unit in balance configuration.
pub const MATERIAL_UNITS_PER_DISPLAY_UNIT: u32 = 100;

/// Converts a whole display quantity to the simulation's integer material units.
pub const fn material_units_from_display(amount: u32) -> Option<u32> {
    amount.checked_mul(MATERIAL_UNITS_PER_DISPLAY_UNIT)
}
