//! World state, terrain, coordinates, generation and navigation.
pub mod coordinates;
pub mod generation;
pub mod map;
pub mod pathfinding;
pub mod province;
pub mod province_graph;
pub mod settlement_environment;
pub mod settlement_registry;
mod state;
pub mod terrain;
pub mod territory;

pub use coordinates::{Point, Position};
pub use map::Map;
pub use province::Province;
pub use province_graph::ProvinceGraph;
pub(crate) use state::NationOperationError;
pub use state::{OwnershipChange, SubsidyTransfer, TaxCollection, TileView, World, WorldError};
