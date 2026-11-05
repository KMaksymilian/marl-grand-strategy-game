//! Applies validated agent decisions through the existing settlement commands.
use super::{World, WorldError};
use crate::settlement::command::SettlementCommand;
use crate::settlement::decision::{
    ActionEffect, ActionError, ActionOutcome, ActionResult, DecisionOutcome, SettlementAction,
    SettlementDecision,
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
                        effect: effect(action, &accepted.command),
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

fn effect(action: &SettlementAction, command: &SettlementCommand) -> ActionEffect {
    match action {
        SettlementAction::Construct { .. } | SettlementAction::Upgrade { .. } => match command {
            SettlementCommand::Construct { project_id, .. }
            | SettlementCommand::Upgrade { project_id, .. } => ActionEffect::ProjectQueued {
                project_id: *project_id,
            },
            other => unreachable!("project action resolved to {other:?}"),
        },
        SettlementAction::CancelProject { project_id } => ActionEffect::ProjectCanceled {
            project_id: *project_id,
        },
        SettlementAction::SetBuildingWorkers {
            building_id,
            workers,
        } => ActionEffect::WorkersSet {
            building_id: *building_id,
            workers: *workers,
        },
        SettlementAction::SetStorageOrder { .. } => ActionEffect::StorageOrderSet,
        SettlementAction::PublishMarketOffers { .. } => ActionEffect::MarketOffersPublished,
        SettlementAction::ClearMarketOffers => ActionEffect::MarketOffersCleared,
        SettlementAction::OrderCaravan { receiver_id, .. } => ActionEffect::CaravanOrdered {
            receiver_id: *receiver_id,
        },
    }
}
