//! Applies validated agent decisions through the existing settlement commands.
use super::{CommandDispatch, preparation::DecisionPreparation};
use crate::application::commands::settlement::contract::{
    ActionError, ActionOutcome, ActionResult, DecisionOutcome, SettlementDecision,
};
use crate::domain::world::WorldError;

/// Validates the decision, then executes the accepted actions in submission
/// order. Rejected actions change nothing. Fails as a whole only when the
/// deciding settlement does not exist; then nothing changes either.
pub(crate) fn handle_decision(
    mut dispatch: CommandDispatch<'_>,
    decision: &SettlementDecision,
) -> Result<DecisionOutcome, WorldError> {
    let validated = DecisionPreparation::new(dispatch.world()).validate_decision(decision)?;
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
        let outcome = match dispatch.execute(validated.settlement_id, &accepted.command) {
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
        settlement_id: validated.settlement_id,
        results,
    })
}
