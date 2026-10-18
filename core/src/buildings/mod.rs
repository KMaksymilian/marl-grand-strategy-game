//! Building instances, shared definitions and travel-cost productivity.
mod building;
pub mod catalog;
pub mod definition;
pub mod productivity;

pub use building::{Building, BuildingUpdateResult, LoadError, ProductionResult};
