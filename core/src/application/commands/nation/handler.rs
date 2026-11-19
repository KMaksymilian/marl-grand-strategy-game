//! Applies validated nation decisions through World's national operations.
use super::contract::{
    NationActionEffect, NationActionError, NationActionOutcome, NationActionResult, NationDecision,
    NationDecisionOutcome,
};
use super::preparation::{NationCommand, validate_nation_decision};
use crate::domain::ids::NationId;
use crate::domain::nation::decree::Decree;
use crate::domain::world::{World, WorldError};

/// Validates the decision, then executes the accepted actions in submission
/// order. Fails as a whole only for an unknown nation; then nothing changes.
pub(crate) fn handle_nation_decision(
    world: &mut World,
    decision: &NationDecision,
) -> Result<NationDecisionOutcome, WorldError> {
    // Step 1: read-only validation of the whole decision.
    let validated = validate_nation_decision(world, decision)?;
    let nation_id = validated.nation_id;
    // Step 2: rejected actions become results right away; they changed nothing.
    let mut results: Vec<_> = validated
        .rejected
        .into_iter()
        .map(|rejected| NationActionResult {
            index: rejected.index,
            kind: rejected.kind,
            outcome: NationActionOutcome::Rejected {
                error: rejected.error,
            },
        })
        .collect();
    // Step 3: accepted actions run in submission order. A failure here means the
    // validator and World disagree: debug builds panic to expose the bug,
    // release builds report `execution_failed` for that action.
    for (index, command) in validated.accepted {
        let outcome = match execute(world, nation_id, command) {
            Ok(effect) => NationActionOutcome::Applied { effect },
            Err(error) => {
                debug_assert!(false, "validated nation action failed: {error:?}");
                NationActionOutcome::Rejected {
                    error: NationActionError::ExecutionFailed,
                }
            }
        };
        results.push(NationActionResult {
            index,
            kind: decision.actions[index].kind(),
            outcome,
        });
    }
    // One result per submitted action, in the agent's original order.
    results.sort_by_key(|result| result.index);
    Ok(NationDecisionOutcome { nation_id, results })
}

fn execute(
    world: &mut World,
    nation_id: NationId,
    command: NationCommand,
) -> Result<NationActionEffect, WorldError> {
    match command {
        NationCommand::SetTaxRate(rate) => {
            world.set_tax_rate(nation_id, rate)?;
            Ok(NationActionEffect::TaxRateSet { rate })
        }
        NationCommand::Subsidize {
            settlement_id,
            gold,
            resources,
        } => {
            let transfer = world.subsidize(nation_id, settlement_id, gold, &resources)?;
            Ok(NationActionEffect::SubsidyDelivered {
                settlement_id,
                gold: transfer.gold,
                delivered: transfer.delivered,
                returned: transfer.returned,
            })
        }
        NationCommand::EnactDecree(decree) => {
            let cost = world.enact_decree(nation_id, decree)?;
            Ok(NationActionEffect::DecreeEnacted { decree, cost })
        }
        NationCommand::RevokeDecree(decree) => {
            world.revoke_decree(nation_id, decree)?;
            Ok(NationActionEffect::DecreeRevoked {
                decree,
                cooldown_turns: Decree::COOLDOWN_TURNS,
            })
        }
        NationCommand::RelocateCapital(settlement_id) => {
            let cost = world.order_capital_relocation(nation_id, settlement_id)?;
            Ok(NationActionEffect::CapitalRelocationOrdered {
                settlement_id,
                cost,
            })
        }
    }
}
