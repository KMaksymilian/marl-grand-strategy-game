//! Building instances, shared definitions and travel-cost productivity.
mod building;
pub mod definition;
pub mod factory;
pub mod productivity;

pub use building::{Building, BuildingUpdateResult, LoadError, Position, ProductionResult};
