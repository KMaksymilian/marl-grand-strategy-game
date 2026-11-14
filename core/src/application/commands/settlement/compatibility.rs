//! Test-only access to decision validation and execution without advancing the world.
use super::DecisionOutcome;
use super::{
    CommandDispatch, SettlementDecision, ValidatedDecision, handler,
    preparation::DecisionPreparation,
};
use crate::domain::world::{World, WorldError};

impl World {
    pub(crate) fn validate_decision(
        &self,
        decision: &SettlementDecision,
    ) -> Result<ValidatedDecision, WorldError> {
        DecisionPreparation::new(self).validate_decision(decision)
    }

    pub(crate) fn apply_decision(
        &mut self,
        decision: &SettlementDecision,
    ) -> Result<DecisionOutcome, WorldError> {
        handler::handle_decision(CommandDispatch::new(self), decision)
    }
}
