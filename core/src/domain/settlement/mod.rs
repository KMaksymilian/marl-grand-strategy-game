//! Settlement aggregate and its domain operations.
mod model;
pub mod start;

pub use model::{
    BuildingCommandError, CaravanOrder, NeedRegistry, NeedTier, NeedsConfigError, Settlement,
    TradeCommandError, projects, turn_report,
};
pub(crate) use model::{CaravanUsage, PreparedTurn, ProducedTurn, offer_units};
