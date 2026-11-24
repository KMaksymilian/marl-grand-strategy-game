//! Serializable results of nation decisions.
use super::contract::NationActionKind;
use crate::domain::economy::ResourceType;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::decree::DecreeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Why one action of a nation decision was rejected. A rejected action changes nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum NationActionError {
    /// The rate is not a fraction from 0.0 to 1.0.
    InvalidTaxRate { rate: f32 },
    /// No settlement with this ID exists.
    UnknownSettlement { settlement_id: SettlementId },
    /// The settlement belongs to another nation.
    SettlementNotOwned { settlement_id: SettlementId },
    /// A subsidy with no gold and no materials.
    EmptySubsidy,
    /// Labour is a per-turn capacity of a population, not a stored good.
    LabourIsNotTransferable,
    /// `available` is the treasury after earlier applied actions.
    InsufficientTreasury { requested: u64, available: u32 },
    /// `available` is the national stock after earlier actual deliveries.
    InsufficientStock {
        resource: ResourceType,
        requested: u64,
        available: u32,
    },
    /// The settlement's gold counter cannot hold the subsidy.
    SettlementTreasuryFull { settlement_id: SettlementId },
    /// No decree with this ID in the decree catalogue.
    UnknownDecree { decree: DecreeId },
    /// The decree is already pending or active.
    DecreeAlreadyEnacted { decree: DecreeId },
    /// The decree was revoked recently; it can be enacted again after
    /// `turns_left` more turns.
    DecreeOnCooldown { decree: DecreeId, turns_left: u32 },
    /// Every decree slot is taken by a pending or active decree.
    NoFreeDecreeSlot { slots: usize },
    /// Only an active decree can be revoked (not a pending one).
    DecreeNotActive { decree: DecreeId },
    /// The decree must stay active for `turns_left` more turns.
    DecreeMinimumDuration { decree: DecreeId, turns_left: u32 },
    /// An earlier action of the same decision already sets the same policy or
    /// targets the same decree.
    Conflict { with_index: usize },
    /// Retained for existing serialized outcomes.
    ExecutionFailed,
}

/// What an applied action changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum NationActionEffect {
    TaxRateSet {
        rate: f32,
    },
    SubsidyDelivered {
        settlement_id: SettlementId,
        gold: u32,
        /// Materials stored in the settlement's warehouse.
        delivered: BTreeMap<ResourceType, u32>,
        /// Materials that did not fit and stayed in the national stock.
        returned: BTreeMap<ResourceType, u32>,
    },
    /// Pending this turn, active from the next; `upkeep` is the gold it will
    /// cost every active turn (for the current number of settlements).
    DecreeEnacted {
        decree: DecreeId,
        upkeep: u64,
    },
    /// Ended now; it can be enacted again after `cooldown_turns` turns.
    DecreeRevoked {
        decree: DecreeId,
        cooldown_turns: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum NationActionOutcome {
    Applied { effect: NationActionEffect },
    Rejected { error: NationActionError },
}

/// Result of one submitted action, identified by its position in the decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NationDecisionOutcome {
    pub nation_id: NationId,
    pub results: Vec<NationActionResult>,
}
