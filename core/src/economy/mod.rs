//! Resource catalogue, stock, currency and allocation rules.
pub mod allocation;
pub mod gold_inventory;
pub mod inventory;
pub mod resources;
pub mod storage_order;

pub use inventory::{CapacityError, InventoryAddResult, ResourceInventory};
pub use resources::ResourceType;
