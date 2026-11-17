//! Resource catalogue, stock, currency and allocation rules.
pub mod allocation;
pub mod cost;
pub mod gold_inventory;
pub mod inventory;
mod resource_catalog;
pub mod resources;
pub mod storage_order;

pub use inventory::{CapacityError, InventoryAddResult, ResourceInventory};
pub use resource_catalog::{
    FoodClass, ResourceCatalog, ResourceCatalogError, ResourceCategory, ResourceDefinition,
};
pub use resources::ResourceType;
