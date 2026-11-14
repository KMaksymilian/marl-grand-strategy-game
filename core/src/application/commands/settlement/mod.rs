//! Settlement command contract, batch validation and execution.
#[cfg(test)]
mod compatibility;
pub mod contract;
mod dispatch;
pub(crate) use dispatch::CommandDispatch;
pub(crate) mod handler;
pub mod input_validation;
mod preparation;
pub(crate) mod prepared;
pub mod response;
pub use input_validation as validation;

pub use contract::*;
#[cfg(test)]
pub(crate) use preparation::ValidatedDecision;
