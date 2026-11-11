//! Game domain and application use cases, independent of rendering.
pub mod application;
pub mod configuration;
pub mod domain;

pub use application::simulation;
pub use domain::{buildings, economy, ids, spatial, trade};

pub mod settlement {
    pub use crate::application::commands::settlement::{contract as decision, prepared as command};
    pub use crate::application::queries::{action_mask, observation};
    pub use crate::domain::settlement::*;
}

pub mod world {
    pub use crate::application::commands::settlement::{
        AcceptedAction, CommandError, ValidatedDecision,
    };
    pub use crate::domain::world::*;
}
