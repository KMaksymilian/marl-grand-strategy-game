//! Settlement aggregate and its domain operations.
mod model;

pub(crate) use model::PreparedTurn;
pub use model::{
    BuildingCommandError, CaravanOrder, NeedRegistry, NeedTier, Settlement, TradeCommandError,
    projects, turn_report,
};
