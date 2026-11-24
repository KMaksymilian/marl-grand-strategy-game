//! Applies each action to the current state before starting the next one.
use super::{
    ActionEffect, ActionError, ActionOutcome, ActionResult, CommandDispatch, DecisionOutcome,
    SettlementAction, SettlementDecision,
};
use crate::domain::ids::{BuildingId, ProjectId};
use crate::domain::spatial::Position;
use crate::domain::world::WorldError;
use std::collections::HashMap;

/// Objects that successful actions may claim once within one decision.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Target {
    Tile(Position),
    Upgrade(BuildingId),
    Project(ProjectId),
    Workers(BuildingId),
    Demolition(BuildingId),
    StorageOrder,
    Market,
}

impl Target {
    fn of(action: &SettlementAction) -> Option<Self> {
        Some(match action {
            SettlementAction::Construct { position, .. } => Self::Tile(*position),
            SettlementAction::Upgrade { building_id, .. } => Self::Upgrade(*building_id),
            SettlementAction::CancelProject { project_id } => Self::Project(*project_id),
            SettlementAction::SetBuildingWorkers { building_id, .. } => Self::Workers(*building_id),
            SettlementAction::SetStorageOrder { .. } => Self::StorageOrder,
            SettlementAction::PublishMarketOffers { .. } | SettlementAction::ClearMarketOffers => {
                Self::Market
            }
            SettlementAction::OrderCaravan { .. } => return None,
            SettlementAction::Demolish { building_id } => Self::Demolition(*building_id),
        })
    }

    fn conflict(self, claimed: &HashMap<Self, usize>) -> Option<usize> {
        let existing = claimed.get(&self).copied();
        if existing.is_some() {
            return existing;
        }
        match self {
            Self::Upgrade(id) | Self::Workers(id) => claimed.get(&Self::Demolition(id)).copied(),
            Self::Demolition(id) => [Self::Upgrade(id), Self::Workers(id)]
                .iter()
                .filter_map(|target| claimed.get(target).copied())
                .min(),
            _ => None,
        }
    }
}

/// Rejected actions leave the world unchanged. A missing deciding settlement
/// rejects the entire decision before dispatch.
pub(crate) fn handle_decision(
    mut dispatch: CommandDispatch<'_>,
    decision: &SettlementDecision,
) -> Result<DecisionOutcome, WorldError> {
    let settlement_id = decision.settlement_id;
    dispatch.require_settlement(settlement_id)?;
    let mut claimed = HashMap::new();
    let mut project_targets = HashMap::new();
    let results = decision
        .actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let target = Target::of(action);
            let outcome = match target.and_then(|target| target.conflict(&claimed)) {
                Some(with_index) => ActionOutcome::Rejected {
                    error: ActionError::Conflict { with_index },
                },
                None => match dispatch.execute(settlement_id, action) {
                    Ok(effect) => {
                        if let ActionEffect::ProjectCanceled { project_id } = effect
                            && let Some(released) = project_targets.remove(&project_id)
                        {
                            claimed.remove(&released);
                        }
                        if let Some(target) = target {
                            claimed.insert(target, index);
                            if let ActionEffect::ProjectQueued { project_id } = effect {
                                project_targets.insert(project_id, target);
                            }
                        }
                        ActionOutcome::Applied { effect }
                    }
                    Err(error) => ActionOutcome::Rejected { error },
                },
            };
            ActionResult {
                index,
                kind: action.kind(),
                outcome,
            }
        })
        .collect();
    Ok(DecisionOutcome {
        settlement_id,
        results,
    })
}
