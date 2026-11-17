//! Serializable results of nation decisions.
use super::contract::NationActionKind;
use crate::domain::economy::ResourceType;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::TaxRate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Why one action of a nation decision was rejected. A rejected action changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum NationActionError {
    /// No settlement with this ID exists.
    UnknownSettlement { settlement_id: SettlementId },
    /// The settlement exists but belongs to another nation or to none.
    SettlementNotOwned { settlement_id: SettlementId },
    /// A subsidy with no gold and no materials.
    EmptySubsidy,
    /// Labour is a per-turn capacity of a population, not a stored good.
    LabourIsNotTransferable,
    /// `available` is the treasury left after earlier subsidies of the decision.
    InsufficientTreasury { requested: u64, available: u32 },
    /// `available` is the national stock left after earlier subsidies of the decision.
    InsufficientStock {
        resource: ResourceType,
        requested: u64,
        available: u32,
    },
    /// The settlement's gold counter cannot hold the subsidy.
    SettlementTreasuryFull { settlement_id: SettlementId },
    /// An earlier action of the same decision already sets the same policy.
    Conflict { with_index: usize },
    /// The engine refused an action that passed validation; the state is unchanged.
    ExecutionFailed,
}

/// What an applied action changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum NationActionEffect {
    TaxRateSet {
        rate: TaxRate,
    },
    SubsidyDelivered {
        settlement_id: SettlementId,
        gold: u32,
        /// Materials stored in the settlement's warehouse.
        delivered: BTreeMap<ResourceType, u32>,
        /// Materials that did not fit and stayed in the national stock.
        returned: BTreeMap<ResourceType, u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum NationActionOutcome {
    Applied { effect: NationActionEffect },
    Rejected { error: NationActionError },
}

/// Result of one submitted action, identified by its position in the decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NationActionResult {
    pub index: usize,
    pub kind: NationActionKind,
    pub outcome: NationActionOutcome,
}

impl NationActionResult {
    pub fn is_applied(&self) -> bool {
        matches!(self.outcome, NationActionOutcome::Applied { .. })
    }
}

/// Results of one nation decision, one entry per action in submission order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NationDecisionOutcome {
    pub nation_id: NationId,
    pub results: Vec<NationActionResult>,
}
