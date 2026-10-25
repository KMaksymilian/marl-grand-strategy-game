//! Mutation commands grouped by affected settlement state.
//! Every operation is still a method on Settlement, not a separate API object.
mod buildings;
mod inventory;
mod projects;
mod trade;

pub use trade::TradeCommandError;
