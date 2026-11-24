//! Settlement command contract and ordered execution.
#[cfg(test)]
mod compatibility;
pub mod contract;
mod dispatch;
pub(crate) use dispatch::CommandDispatch;
pub(crate) mod handler;
pub mod input_validation;
pub mod response;
pub use input_validation as validation;

pub use contract::*;
