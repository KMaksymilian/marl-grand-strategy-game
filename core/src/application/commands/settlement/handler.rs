//! Applies validated agent decisions through the existing settlement commands.
use super::{dispatch, preparation::DecisionPreparation};
use crate::application::commands::settlement::contract::{
    ActionError, ActionOutcome, ActionResult, DecisionOutcome, SettlementDecision,
};
use crate::domain::world::{World, WorldError};

/// Validates the decision, then executes the accepted actions in submission
/// order. Rejected actions change nothing. Fails as a whole only when the
/// deciding settlement does not exist; then nothing changes either.
pub(crate) fn handle_decision(
    world: &mut World,
    decision: &SettlementDecision,
) -> Result<DecisionOutcome, WorldError> {
    let validated = DecisionPreparation::new(world).validate_decision(decision)?;
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
        let outcome = match dispatch::execute(world, decision.settlement_id, &accepted.command) {
            Ok(()) => ActionOutcome::Applied {
                effect: accepted.effect,
            },
            Err(_) => ActionOutcome::Rejected {
                error: ActionError::ExecutionFailed,
            },
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
