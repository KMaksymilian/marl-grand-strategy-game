//! Test-only execution of a decision without advancing the world.
use super::{CommandDispatch, DecisionOutcome, SettlementDecision, handler};
use crate::domain::world::{World, WorldError};

impl World {
    pub(crate) fn apply_decision(
        &mut self,
        decision: &SettlementDecision,
    ) -> Result<DecisionOutcome, WorldError> {
        handler::handle_decision(CommandDispatch::new(self), decision)
    }
}
