//! Applies validated agent decisions through the existing settlement commands.
use super::{World, WorldError};
use crate::settlement::decision::{
    ActionError, ActionOutcome, ActionResult, DecisionOutcome, SettlementDecision,
};

impl World {
    /// Validates the decision, then executes the accepted actions in submission
    /// order. Rejected actions change nothing. Fails as a whole only when the
    /// deciding settlement does not exist; then nothing changes either.
    pub fn apply_decision(
        &mut self,
        decision: &SettlementDecision,
    ) -> Result<DecisionOutcome, WorldError> {
        let validated = self.validate_decision(decision)?;
        let mut results: Vec<ActionResult> = validated
            .rejected
            .into_iter()
            .map(|rejected| ActionResult {
                index: rejected.index,
                kind: rejected.kind,
                outcome: ActionOutcome::Rejected {
                    error: rejected.error,
                },
            })
            .collect();
        for accepted in validated.accepted {
            let action = &decision.actions[accepted.index];
            let outcome =
                match self.execute_settlement_command(decision.settlement_id, &accepted.command) {
                    Ok(()) => ActionOutcome::Applied {
                        effect: accepted.effect,
                    },
                    Err(error) => {
                        debug_assert!(false, "validated action failed: {error:?}");
                        ActionOutcome::Rejected {
                            error: ActionError::ExecutionFailed,
                        }
                    }
                };
            results.push(ActionResult {
                index: accepted.index,
                kind: action.kind(),
                outcome,
            });
        }
        results.sort_by_key(|result| result.index);
        Ok(DecisionOutcome {
            settlement_id: decision.settlement_id,
            results,
        })
    }
}
