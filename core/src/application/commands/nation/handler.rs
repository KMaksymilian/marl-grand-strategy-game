//! Applies nation actions in submission order against the authoritative world.
use super::contract::{
    NationAction, NationActionEffect, NationActionError, NationActionOutcome, NationActionResult,
    NationDecision, NationDecisionOutcome,
};
use crate::domain::ids::NationId;
use crate::domain::nation::diplomacy::{DiplomaticOutcome, Treaty};
use crate::domain::nation::{Nation, decree::DecreeId};
use crate::domain::world::{NationOperationError, World, WorldError};
use std::collections::BTreeMap;

/// Policies, decrees and other nations that a decision may change once.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum Target<'a> {
    TaxRate,
    Decree(&'a DecreeId),
    /// At most one diplomatic action towards each other nation (§8).
    Nation(NationId),
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
            NationAction::DeclareWar { target }
            | NationAction::DeclareEmbargo { target }
            | NationAction::LiftEmbargo { target }
            | NationAction::BreakTradeAgreement { target }
            | NationAction::BreakAlliance { target }
            | NationAction::ProposeTreaty { target, .. } => Some(Self::Nation(*target)),
            NationAction::RespondToProposal { proposer, .. } => Some(Self::Nation(*proposer)),
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
        NationAction::DeclareWar { target } => diplomacy(world.declare_war(nation_id, *target)),
        NationAction::DeclareEmbargo { target } => {
            diplomacy(world.declare_embargo(nation_id, *target))
        }
        NationAction::LiftEmbargo { target } => diplomacy(world.lift_embargo(nation_id, *target)),
        NationAction::BreakTradeAgreement { target } => {
            diplomacy(world.break_treaty(nation_id, *target, Treaty::TradeAgreement))
        }
        NationAction::BreakAlliance { target } => {
            diplomacy(world.break_treaty(nation_id, *target, Treaty::Alliance))
        }
        NationAction::ProposeTreaty { target, treaty } => {
            diplomacy(world.propose_treaty(nation_id, *target, *treaty))
        }
        NationAction::RespondToProposal {
            proposer,
            treaty,
            accept,
        } => diplomacy(world.respond_to_proposal(nation_id, *proposer, *treaty, *accept)),
        NationAction::RecruitUnits {
            settlement_id,
            unit,
            units,
        } => {
            let order = world.recruit_units(nation_id, *settlement_id, unit, *units)?;
            Ok(NationActionEffect::UnitsRecruited {
                settlement_id: *settlement_id,
                unit: unit.clone(),
                units: *units,
                soldiers: order.soldiers,
                ready_in_turns: order.ready_in_turns,
                army_id: order.army_id,
            })
        }
        NationAction::DisbandArmy { army_id } => {
            let (settlement_id, soldiers) = world.disband_army(nation_id, *army_id)?;
            Ok(NationActionEffect::ArmyDisbanded {
                army_id: *army_id,
                settlement_id,
                soldiers,
            })
        }
        NationAction::MergeArmies { army_id, into } => {
            world.merge_armies(nation_id, *army_id, *into)?;
            Ok(NationActionEffect::ArmiesMerged {
                army_id: *army_id,
                into: *into,
            })
        }
        NationAction::SplitArmy { army_id, units } => {
            let new_army_id = world.split_army(nation_id, *army_id, units)?;
            Ok(NationActionEffect::ArmySplit {
                army_id: *army_id,
                new_army_id,
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
        Domain::Diplomacy(error) => Action::Diplomacy { error },
        Domain::UnknownUnitType(unit) => Action::UnknownUnitType { unit },
        Domain::EmptyRecruitment => Action::EmptyRecruitment,
        Domain::ConscriptionPoolExceeded {
            settlement_id,
            requested,
            available,
        } => Action::ConscriptionPoolExceeded {
            settlement_id,
            requested,
            available,
        },
        Domain::UnknownArmy(army_id) => Action::UnknownArmy { army_id },
        Domain::ArmyNotOwned(army_id) => Action::ArmyNotOwned { army_id },
        Domain::NotOwnProvince { army_id, province } => {
            Action::NotOwnProvince { army_id, province }
        }
        Domain::ArmiesApart { army_id, into } => Action::ArmiesApart { army_id, into },
        Domain::SameArmy(army_id) => Action::SameArmy { army_id },
        Domain::InvalidUnitSelection(army_id) => Action::InvalidUnitSelection { army_id },
        Domain::ArmyIdExhausted => Action::ArmyIdsExhausted,
    })
}

fn diplomacy(
    outcome: Result<DiplomaticOutcome, NationOperationError>,
) -> Result<NationActionEffect, NationOperationError> {
    outcome.map(|outcome| NationActionEffect::Diplomacy { outcome })
}
