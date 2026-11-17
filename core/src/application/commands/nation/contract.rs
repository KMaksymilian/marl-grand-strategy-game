//! Serializable decisions submitted by nation agents (gameplay specification §5.2).
//!
//! The contract covers the policy and orders implemented so far; decrees,
//! recruitment, army orders and diplomacy are added as new action variants.
//! Values are checked against the world before execution; deserialization checks
//! only the shape and numeric types.
//!
//! JSON shape:
//! ```json
//! {
//!   "nation_id": 0,
//!   "actions": [
//!     { "type": "set_tax_rate", "rate": 0.15 },
//!     { "type": "subsidize", "settlement_id": 2, "gold": 50, "resources": { "Wood": 20 } },
//!     { "type": "enact_decree", "decree": "agricultural_decree" },
//!     { "type": "revoke_decree", "decree": "games" },
//!     { "type": "relocate_capital", "settlement_id": 3 },
//!     { "type": "declare_war", "target": 1 },
//!     { "type": "propose_treaty", "target": 2, "treaty": "trade_agreement" },
//!     { "type": "respond_to_proposal", "proposer": 2, "treaty": "peace", "accept": true }
//!   ]
//! }
//! ```
pub use super::response::{
    NationActionEffect, NationActionError, NationActionOutcome, NationActionResult,
    NationDecisionOutcome,
};
use crate::domain::economy::ResourceType;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::TaxRate;
use crate::domain::nation::decree::Decree;
use crate::domain::nation::diplomacy::{DiplomaticAction, Treaty};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// All actions one nation submits for one turn, applied in list order. Each one
/// is accepted or rejected on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NationDecision {
    pub nation_id: NationId,
    #[serde(default)]
    pub actions: Vec<NationAction>,
}

impl NationDecision {
    pub fn new(nation_id: NationId, actions: Vec<NationAction>) -> Self {
        Self { nation_id, actions }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("decision values always serialize")
    }

    pub fn from_json(source: &str) -> Result<Self, NationDecisionParseError> {
        serde_json::from_str(source).map_err(NationDecisionParseError)
    }
}

/// One nation decision. Settlements are referenced by ID; amounts are whole units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NationAction {
    /// The common tax rate of all own settlements, as a fraction: 0.0 is 0%, 0.1 is 10%, 1.0 is 100%.
    SetTaxRate { rate: TaxRate },
    /// Move gold and materials from the national treasury and stock to an own
    /// settlement. Materials that do not fit its warehouse stay national.
    Subsidize {
        settlement_id: SettlementId,
        #[serde(default)]
        gold: u32,
        #[serde(default)]
        resources: BTreeMap<ResourceType, u32>,
    },
    /// Start a decree: it takes a slot now, works from the next turn and costs
    /// its upkeep every turn it works (§7).
    EnactDecree { decree: Decree },
    /// End a decree that has been active for at least five turns; it can be
    /// enacted again after a five-turn cooldown.
    RevokeDecree { decree: Decree },
    /// One-shot decree: pay now, and the capital moves to this own settlement
    /// at the start of the next turn if it is still owned and not under attack;
    /// otherwise the cost is refunded. Uses no slot.
    RelocateCapital { settlement_id: SettlementId },
    /// Start a war; only from neutrality or an embargo, not during a truce
    /// and not in the turn a treaty with the target was broken. Allies of the
    /// target join the war (§8.3).
    DeclareWar { target: NationId },
    /// Block trade without war; only from neutrality.
    DeclareEmbargo { target: NationId },
    /// End an own embargo; back to neutrality.
    LiftEmbargo { target: NationId },
    /// Back to neutrality; counts against the reputation (§8.4).
    BreakTradeAgreement { target: NationId },
    /// Back to neutrality; counts against the reputation (§8.4).
    BreakAlliance { target: NationId },
    /// Offer a trade agreement, an alliance or peace; the target answers in
    /// the next turn, otherwise the proposal expires.
    ProposeTreaty { target: NationId, treaty: Treaty },
    /// Answer a proposal made in the previous turn.
    RespondToProposal {
        proposer: NationId,
        treaty: Treaty,
        accept: bool,
    },
}

impl NationAction {
    pub fn kind(&self) -> NationActionKind {
        match self {
            Self::SetTaxRate { .. } => NationActionKind::SetTaxRate,
            Self::Subsidize { .. } => NationActionKind::Subsidize,
            Self::EnactDecree { .. } => NationActionKind::EnactDecree,
            Self::RevokeDecree { .. } => NationActionKind::RevokeDecree,
            Self::RelocateCapital { .. } => NationActionKind::RelocateCapital,
            Self::DeclareWar { .. } => NationActionKind::DeclareWar,
            Self::DeclareEmbargo { .. } => NationActionKind::DeclareEmbargo,
            Self::LiftEmbargo { .. } => NationActionKind::LiftEmbargo,
            Self::BreakTradeAgreement { .. } => NationActionKind::BreakTradeAgreement,
            Self::BreakAlliance { .. } => NationActionKind::BreakAlliance,
            Self::ProposeTreaty { .. } => NationActionKind::ProposeTreaty,
            Self::RespondToProposal { .. } => NationActionKind::RespondToProposal,
        }
    }

    /// The domain action of a diplomatic variant; None for other actions.
    pub fn diplomatic(&self) -> Option<DiplomaticAction> {
        Some(match *self {
            Self::DeclareWar { target } => DiplomaticAction::DeclareWar { target },
            Self::DeclareEmbargo { target } => DiplomaticAction::DeclareEmbargo { target },
            Self::LiftEmbargo { target } => DiplomaticAction::LiftEmbargo { target },
            Self::BreakTradeAgreement { target } => {
                DiplomaticAction::BreakTradeAgreement { target }
            }
            Self::BreakAlliance { target } => DiplomaticAction::BreakAlliance { target },
            Self::ProposeTreaty { target, treaty } => DiplomaticAction::Propose { target, treaty },
            Self::RespondToProposal {
                proposer,
                treaty,
                accept,
            } => DiplomaticAction::Respond {
                proposer,
                treaty,
                accept,
            },
            _ => return None,
        })
    }
}

/// Action variant without parameters. Serialized names match the `type` tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NationActionKind {
    SetTaxRate,
    Subsidize,
    EnactDecree,
    RevokeDecree,
    RelocateCapital,
    DeclareWar,
    DeclareEmbargo,
    LiftEmbargo,
    BreakTradeAgreement,
    BreakAlliance,
    ProposeTreaty,
    RespondToProposal,
}

impl NationActionKind {
    /// Every kind in a fixed order, e.g. for a fixed-size mask vector.
    pub const ALL: [Self; 12] = [
        Self::SetTaxRate,
        Self::Subsidize,
        Self::EnactDecree,
        Self::RevokeDecree,
        Self::RelocateCapital,
        Self::DeclareWar,
        Self::DeclareEmbargo,
        Self::LiftEmbargo,
        Self::BreakTradeAgreement,
        Self::BreakAlliance,
        Self::ProposeTreaty,
        Self::RespondToProposal,
    ];
}

/// The JSON text is malformed or does not match the nation decision contract.
#[derive(Debug)]
pub struct NationDecisionParseError(serde_json::Error);

impl fmt::Display for NationDecisionParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid nation decision: {}", self.0)
    }
}

impl std::error::Error for NationDecisionParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ResourceType::{Stone, Wood};

    fn one_of_each() -> Vec<NationAction> {
        vec![
            NationAction::SetTaxRate {
                rate: TaxRate::from_fraction(0.15).unwrap(),
            },
            NationAction::Subsidize {
                settlement_id: SettlementId(2),
                gold: 50,
                resources: BTreeMap::from([(Wood, 20), (Stone, 5)]),
            },
            NationAction::EnactDecree {
                decree: Decree::AgriculturalDecree,
            },
            NationAction::RevokeDecree {
                decree: Decree::Games,
            },
            NationAction::RelocateCapital {
                settlement_id: SettlementId(3),
            },
            NationAction::DeclareWar {
                target: NationId(1),
            },
            NationAction::DeclareEmbargo {
                target: NationId(1),
            },
            NationAction::LiftEmbargo {
                target: NationId(1),
            },
            NationAction::BreakTradeAgreement {
                target: NationId(1),
            },
            NationAction::BreakAlliance {
                target: NationId(1),
            },
            NationAction::ProposeTreaty {
                target: NationId(2),
                treaty: Treaty::TradeAgreement,
            },
            NationAction::RespondToProposal {
                proposer: NationId(2),
                treaty: Treaty::Peace,
                accept: true,
            },
        ]
    }

    #[test]
    fn every_action_survives_a_json_round_trip() {
        let actions = one_of_each();
        let kinds: Vec<_> = actions.iter().map(NationAction::kind).collect();
        assert_eq!(kinds, NationActionKind::ALL);
        let decision = NationDecision::new(NationId(3), actions);
        assert_eq!(
            NationDecision::from_json(&decision.to_json()).unwrap(),
            decision
        );
    }

    #[test]
    fn json_shape_uses_snake_case_tags_and_fractional_rates() {
        let decision = NationDecision::new(NationId(0), one_of_each());
        let expected = serde_json::json!({
            "nation_id": 0,
            "actions": [
                {"type": "set_tax_rate", "rate": 0.15},
                {"type": "subsidize", "settlement_id": 2, "gold": 50,
                 "resources": {"Stone": 5, "Wood": 20}},
                {"type": "enact_decree", "decree": "agricultural_decree"},
                {"type": "revoke_decree", "decree": "games"},
                {"type": "relocate_capital", "settlement_id": 3},
                {"type": "declare_war", "target": 1},
                {"type": "declare_embargo", "target": 1},
                {"type": "lift_embargo", "target": 1},
                {"type": "break_trade_agreement", "target": 1},
                {"type": "break_alliance", "target": 1},
                {"type": "propose_treaty", "target": 2, "treaty": "trade_agreement"},
                {"type": "respond_to_proposal", "proposer": 2, "treaty": "peace", "accept": true}
            ]
        });
        assert_eq!(serde_json::to_value(&decision).unwrap(), expected);
        for (action, kind) in decision.actions.iter().zip(NationActionKind::ALL) {
            let tag = serde_json::to_value(action).unwrap()["type"].clone();
            assert_eq!(tag, serde_json::to_value(kind).unwrap());
        }
    }

    #[test]
    fn subsidy_amounts_default_to_zero_and_actions_to_none() {
        let decision = NationDecision::from_json(
            r#"{"nation_id": 1, "actions": [{"type": "subsidize", "settlement_id": 4}]}"#,
        )
        .unwrap();
        assert_eq!(
            decision.actions,
            vec![NationAction::Subsidize {
                settlement_id: SettlementId(4),
                gold: 0,
                resources: BTreeMap::new(),
            }]
        );
        assert_eq!(
            NationDecision::from_json(r#"{"nation_id": 1}"#).unwrap(),
            NationDecision::new(NationId(1), vec![])
        );
    }

    #[test]
    fn malformed_decisions_are_rejected_when_parsing() {
        let wrap = |action: &str| format!(r#"{{"nation_id": 1, "actions": [{action}]}}"#);
        let cases = [
            wrap(r#"{"type": "set_tax_rate", "rate": 1.5}"#),
            wrap(r#"{"type": "set_tax_rate", "rate": -0.1}"#),
            wrap(r#"{"type": "set_tax_rate", "rate": "0.1"}"#),
            wrap(r#"{"type": "set_tax_rate"}"#),
            wrap(r#"{"type": "subsidize", "settlement_id": 1, "gold": -1}"#),
            wrap(r#"{"type": "subsidize", "settlement_id": 1, "resources": {"Gold": 1}}"#),
            wrap(r#"{"type": "subsidize", "settlement_id": 1, "army": 3}"#),
            wrap(r#"{"type": "declare_war", "nation": 2}"#),
            wrap(r#"{"type": "propose_treaty", "target": 2, "treaty": "marriage"}"#),
            wrap(r#"{"type": "respond_to_proposal", "proposer": 2, "treaty": "peace"}"#),
            wrap(r#"{"type": "enact_decree", "decree": "free_beer"}"#),
            wrap(r#"{"type": "enact_decree"}"#),
            wrap(r#"{"type": "relocate_capital", "settlement_id": -1}"#),
            r#"{"nation_id": -1}"#.to_owned(),
            r#"{"nation_id": 1, "turn": 2}"#.to_owned(),
            r#"{"actions": []}"#.to_owned(),
            "[]".to_owned(),
        ];
        for source in cases {
            let error = NationDecision::from_json(&source);
            assert!(error.is_err(), "accepted: {source}");
            assert!(
                error
                    .unwrap_err()
                    .to_string()
                    .starts_with("invalid nation decision")
            );
        }
    }
}
