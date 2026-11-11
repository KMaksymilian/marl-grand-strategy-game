//! Settlement command contract, batch validation and execution.
mod compatibility;
pub mod contract;
mod dispatch;
pub(crate) mod handler;
pub(in crate::application) mod preparation;
pub mod prepared;
pub mod response;
pub mod validation;

pub use contract::*;
pub use dispatch::CommandError;
pub use preparation::{AcceptedAction, ValidatedDecision};
