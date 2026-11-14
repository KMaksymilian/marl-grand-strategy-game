//! Game domain and application use cases, independent of rendering.
pub mod application;
pub mod configuration;
pub mod domain;

#[cfg(test)]
extern crate self as simulation_core;
#[cfg(test)]
mod test_support;

pub use application::simulation;
pub use domain::{buildings, economy, ids, spatial, trade};

pub mod settlement {
    pub use crate::application::commands::settlement::contract as decision;
    #[cfg(test)]
    pub(crate) use crate::application::commands::settlement::prepared as command;
    pub use crate::application::queries::observation;
    pub use crate::domain::settlement::*;
}

pub mod world {
    #[cfg(test)]
    pub(crate) use crate::application::commands::settlement::ValidatedDecision;
    pub use crate::domain::world::*;
}
