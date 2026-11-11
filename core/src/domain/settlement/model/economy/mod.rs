//! Settlement-local collections and demand/stock processing.
//! Shared resource primitives and allocation algorithms live in crate::domain::economy.
mod buildings;
mod inventory;
mod population;

pub(super) use buildings::BuildingManager;
pub(super) use inventory::SettlementInventoryManager;
pub(super) use population::PopulationManager;
pub use population::{ConsumptionResult, NeedRegistry, NeedTier};
