use super::DecisionOutcome;
use super::{
    CommandError, SettlementDecision, ValidatedDecision, dispatch, handler,
    preparation::DecisionPreparation, prepared::SettlementCommand,
};
use crate::domain::{
    ids::SettlementId,
    world::{World, WorldError},
};

impl World {
    pub fn validate_decision(
        &self,
        decision: &SettlementDecision,
    ) -> Result<ValidatedDecision, WorldError> {
        DecisionPreparation::new(self).validate_decision(decision)
    }

    pub fn apply_decision(
        &mut self,
        decision: &SettlementDecision,
    ) -> Result<DecisionOutcome, WorldError> {
        handler::handle_decision(self, decision)
    }

    pub fn execute_settlement_command(
        &mut self,
        id: SettlementId,
        command: &SettlementCommand,
    ) -> Result<(), CommandError> {
        dispatch::execute(self, id, command)
    }
}
