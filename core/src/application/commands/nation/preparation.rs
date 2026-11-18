//! Read-only validation of nation decisions against the authoritative world.
//!
//! Policy (the same as for settlements): actions are checked in submission order
//! and each one is accepted or rejected on its own. Subsidies of one decision
//! share the treasury and stock they draw from, so each one is checked against
//! what earlier accepted subsidies left. Validation never changes the world.
use super::contract::{NationAction, NationActionError, NationActionKind, NationDecision};
use crate::domain::economy::ResourceType;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::TaxRate;
use crate::domain::world::{World, WorldError};
use std::collections::BTreeMap;

/// An accepted action resolved to the operation that executes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NationCommand {
    SetTaxRate(TaxRate),
    Subsidize {
        settlement_id: SettlementId,
        gold: u32,
        /// Only non-zero amounts.
        resources: BTreeMap<ResourceType, u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RejectedNationAction {
    pub index: usize,
    pub kind: NationActionKind,
    pub error: NationActionError,
}

/// Every action index appears exactly once, in `accepted` or in `rejected`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedNationDecision {
    pub nation_id: NationId,
    pub accepted: Vec<(usize, NationCommand)>,
    pub rejected: Vec<RejectedNationAction>,
}

/// What the actions accepted so far have claimed. Validation works on this copy
/// instead of the world, so it stays read-only and later actions see what
/// earlier accepted ones will use.
struct Batch {
    /// Index of the accepted `SetTaxRate`, to reject a second one as a conflict.
    tax_rate_index: Option<usize>,
    /// Treasury not yet promised to earlier subsidies.
    gold_left: u32,
    /// National stock not yet promised to earlier subsidies.
    stock_left: BTreeMap<ResourceType, u32>,
    /// Gold already promised to each settlement, for the u32 overflow check.
    gold_sent: BTreeMap<SettlementId, u64>,
}

/// Fails as a whole only for an unknown nation; then nothing is validated.
pub(crate) fn validate_nation_decision(
    world: &World,
    decision: &NationDecision,
) -> Result<ValidatedNationDecision, WorldError> {
    let nation_id = decision.nation_id;
    let nation = world
        .nation(nation_id)
        .ok_or(WorldError::MissingNation(nation_id))?;
    let mut batch = Batch {
        tax_rate_index: None,
        gold_left: nation.treasury(),
        stock_left: nation.stock().resources().collect(),
        gold_sent: BTreeMap::new(),
    };
    let mut validated = ValidatedNationDecision {
        nation_id,
        accepted: Vec::new(),
        rejected: Vec::new(),
    };
    for (index, action) in decision.actions.iter().enumerate() {
        match check_action(world, nation_id, &mut batch, index, action) {
            Ok(command) => validated.accepted.push((index, command)),
            Err(error) => validated.rejected.push(RejectedNationAction {
                index,
                kind: action.kind(),
                error,
            }),
        }
    }
    Ok(validated)
}

/// Checks one action and, when it is accepted, records what it claims.
fn check_action(
    world: &World,
    nation_id: NationId,
    batch: &mut Batch,
    index: usize,
    action: &NationAction,
) -> Result<NationCommand, NationActionError> {
    match action {
        NationAction::SetTaxRate { rate } => {
            if let Some(with_index) = batch.tax_rate_index {
                return Err(NationActionError::Conflict { with_index });
            }
            batch.tax_rate_index = Some(index);
            Ok(NationCommand::SetTaxRate(*rate))
        }
        NationAction::Subsidize {
            settlement_id,
            gold,
            resources,
        } => {
            let settlement_id = *settlement_id;
            // Order of checks: the target (exists, is ours), then the content,
            // then the budget. The first failing check is the reported error.
            let settlement = world
                .settlement(settlement_id)
                .ok_or(NationActionError::UnknownSettlement { settlement_id })?;
            if world.settlement_owner(settlement_id) != Some(nation_id) {
                return Err(NationActionError::SettlementNotOwned { settlement_id });
            }
            if resources.contains_key(&ResourceType::Labour) {
                return Err(NationActionError::LabourIsNotTransferable);
            }
            // Zero amounts parse but carry nothing; dropping them keeps effects
            // and reports free of empty entries.
            let resources: BTreeMap<_, _> = resources
                .iter()
                .filter(|(_, amount)| **amount > 0)
                .map(|(&resource, &amount)| (resource, amount))
                .collect();
            if *gold == 0 && resources.is_empty() {
                return Err(NationActionError::EmptySubsidy);
            }
            if *gold > batch.gold_left {
                return Err(NationActionError::InsufficientTreasury {
                    requested: u64::from(*gold),
                    available: batch.gold_left,
                });
            }
            for (&resource, &amount) in &resources {
                let available = batch.stock_left.get(&resource).copied().unwrap_or(0);
                if amount > available {
                    return Err(NationActionError::InsufficientStock {
                        resource,
                        requested: u64::from(amount),
                        available,
                    });
                }
            }
            let sent = batch.gold_sent.get(&settlement_id).copied().unwrap_or(0);
            if u64::from(settlement.gold()) + sent + u64::from(*gold) > u64::from(u32::MAX) {
                return Err(NationActionError::SettlementTreasuryFull { settlement_id });
            }

            // Accepted: claim the budget. Rejected actions never get here, so
            // they claim nothing.
            batch.gold_left -= gold;
            for (&resource, &amount) in &resources {
                *batch.stock_left.get_mut(&resource).expect("checked above") -= amount;
            }
            *batch.gold_sent.entry(settlement_id).or_insert(0) += u64::from(*gold);
            Ok(NationCommand::Subsidize {
                settlement_id,
                gold: *gold,
                resources,
            })
        }
    }
}
