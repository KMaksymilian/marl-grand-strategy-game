//! Applies nation actions in submission order against the authoritative world.
use super::contract::{
    NationAction, NationActionEffect, NationActionError, NationActionOutcome, NationActionResult,
    NationDecision, NationDecisionOutcome,
};
use crate::domain::ids::NationId;
use crate::domain::nation::{Nation, decree::DecreeId};
use crate::domain::world::{NationOperationError, World, WorldError};
use std::collections::BTreeMap;

/// Policies and decrees that a decision may change once.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum Target<'a> {
    TaxRate,
    Decree(&'a DecreeId),
}

impl<'a> Target<'a> {
    fn of(action: &'a NationAction) -> Option<Self> {
        match action {
            NationAction::SetTaxRate { rate } if Nation::is_valid_tax_rate(*rate) => {
                Some(Self::TaxRate)
            }
            NationAction::EnactDecree { decree } | NationAction::RevokeDecree { decree } => {
                Some(Self::Decree(decree))
            }
            _ => None,
        }
    }
}

/// Each applied action becomes visible to the next. Rejected actions claim no target.
pub(crate) fn handle_nation_decision(
    world: &mut World,
    decision: &NationDecision,
) -> Result<NationDecisionOutcome, WorldError> {
    let nation_id = decision.nation_id;
    world
        .nation(nation_id)
        .ok_or(WorldError::MissingNation(nation_id))?;

    let mut claims = BTreeMap::new();
    let mut results = Vec::with_capacity(decision.actions.len());
    for (index, action) in decision.actions.iter().enumerate() {
        let target = Target::of(action);
        let outcome =
            if let Some(with_index) = target.and_then(|target| claims.get(&target).copied()) {
                NationActionOutcome::Rejected {
                    error: NationActionError::Conflict { with_index },
                }
            } else {
                match execute(world, nation_id, action) {
                    Ok(effect) => {
                        if let Some(target) = target {
                            claims.insert(target, index);
                        }
                        NationActionOutcome::Applied { effect }
                    }
                    Err(error) => NationActionOutcome::Rejected {
                        error: report_error(error)?,
                    },
                }
            };
        results.push(NationActionResult {
            index,
            kind: action.kind(),
            outcome,
        });
    }
    Ok(NationDecisionOutcome { nation_id, results })
}

fn execute(
    world: &mut World,
    nation_id: NationId,
    action: &NationAction,
) -> Result<NationActionEffect, NationOperationError> {
    match action {
        NationAction::SetTaxRate { rate } => {
            world.set_tax_rate(nation_id, *rate)?;
            Ok(NationActionEffect::TaxRateSet { rate: *rate })
        }
        NationAction::Subsidize {
            settlement_id,
            gold,
            resources,
        } => {
            let transfer = world.subsidize(nation_id, *settlement_id, *gold, resources)?;
            Ok(NationActionEffect::SubsidyDelivered {
                settlement_id: *settlement_id,
                gold: transfer.gold,
                delivered: transfer.delivered,
                returned: transfer.returned,
            })
        }
        NationAction::EnactDecree { decree } => {
            let upkeep = world.enact_decree(nation_id, decree.clone())?;
            Ok(NationActionEffect::DecreeEnacted {
                decree: decree.clone(),
                upkeep,
            })
        }
        NationAction::RevokeDecree { decree } => {
            let cooldown_turns = world.revoke_decree(nation_id, decree)?;
            Ok(NationActionEffect::DecreeRevoked {
                decree: decree.clone(),
                cooldown_turns,
            })
        }
    }
}

fn report_error(error: NationOperationError) -> Result<NationActionError, WorldError> {
    use NationActionError as Action;
    use NationOperationError as Domain;

    Ok(match error {
        Domain::MissingNation(nation_id) => return Err(WorldError::MissingNation(nation_id)),
        Domain::InvalidTaxRate(rate) => Action::InvalidTaxRate { rate },
        Domain::UnknownSettlement(settlement_id) => Action::UnknownSettlement { settlement_id },
        Domain::SettlementNotOwned(settlement_id) => Action::SettlementNotOwned { settlement_id },
        Domain::EmptySubsidy => Action::EmptySubsidy,
        Domain::LabourIsNotTransferable => Action::LabourIsNotTransferable,
        Domain::InsufficientTreasury {
            requested,
            available,
        } => Action::InsufficientTreasury {
            requested: u64::from(requested),
            available,
        },
        Domain::InsufficientStock {
            resource,
            requested,
            available,
        } => Action::InsufficientStock {
            resource,
            requested: u64::from(requested),
            available,
        },
        Domain::UnknownDecree(decree) => Action::UnknownDecree { decree },
        Domain::DecreeAlreadyEnacted(decree) => Action::DecreeAlreadyEnacted { decree },
        Domain::DecreeOnCooldown { decree, turns_left } => {
            Action::DecreeOnCooldown { decree, turns_left }
        }
        Domain::NoFreeDecreeSlot(slots) => Action::NoFreeDecreeSlot { slots },
        Domain::DecreeNotActive(decree) => Action::DecreeNotActive { decree },
        Domain::DecreeMinimumDuration { decree, turns_left } => {
            Action::DecreeMinimumDuration { decree, turns_left }
        }
    })
}
