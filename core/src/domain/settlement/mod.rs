//! Settlement aggregate and its domain operations.
mod model;

pub use model::{
    BuildingCommandError, CaravanOrder, NeedRegistry, NeedTier, Settlement, TradeCommandError,
    projects, turn_report,
};
pub(crate) use model::{CaravanUsage, PreparedTurn, ProducedTurn, offer_units};
