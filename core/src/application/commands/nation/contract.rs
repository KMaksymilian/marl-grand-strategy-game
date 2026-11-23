//! Serializable decisions submitted by nation agents (gameplay specification §5.2).
//!
//! The contract holds the policy and orders implemented so far; diplomacy,
//! recruitment and army orders are added as new action variants. Decrees are
//! named by their ID from `decrees.toml`; whether the ID exists is checked by
//! validation, not by parsing.
//! Values are checked against the world before execution; deserialization checks
//! only the shape and the numeric types.
//!
//! JSON shape:
//! ```json
//! {
//!   "nation_id": 0,
//!   "actions": [
//!     { "type": "set_tax_rate", "rate": 0.15 },
//!     { "type": "subsidize", "settlement_id": 2, "gold": 50, "resources": { "Wood": 20 } },
//!     { "type": "enact_decree", "decree": "agricultural_decree" },
//!     { "type": "revoke_decree", "decree": "games" }
//!   ]
//! }
//! ```
pub use super::response::{
    NationActionEffect, NationActionError, NationActionOutcome, NationActionResult,
    NationDecisionOutcome,
};
use crate::domain::economy::ResourceType;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::decree::DecreeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// All actions one nation submits for one turn, applied in list order. Each one
/// is accepted or rejected on its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NationAction {
    /// The common tax rate of all own settlements, any fraction from 0.0 (0%)
    /// to 1.0 (100%); other values are rejected by validation.
    SetTaxRate { rate: f32 },
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
    /// its upkeep every active turn (§7). Enacting itself costs nothing.
    EnactDecree { decree: DecreeId },
    /// End a decree active for the minimum duration; it can be enacted again
    /// after the cooldown. Nothing is refunded.
    RevokeDecree { decree: DecreeId },
}

impl NationAction {
    pub fn kind(&self) -> NationActionKind {
        match self {
            Self::SetTaxRate { .. } => NationActionKind::SetTaxRate,
            Self::Subsidize { .. } => NationActionKind::Subsidize,
            Self::EnactDecree { .. } => NationActionKind::EnactDecree,
            Self::RevokeDecree { .. } => NationActionKind::RevokeDecree,
        }
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
}

impl NationActionKind {
    /// Every kind in a fixed order, e.g. for a fixed-size mask vector.
    pub const ALL: [Self; 4] = [
        Self::SetTaxRate,
        Self::Subsidize,
        Self::EnactDecree,
        Self::RevokeDecree,
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

    fn one_of_each() -> Vec<NationAction> {
        vec![
            NationAction::SetTaxRate { rate: 0.15 },
            NationAction::Subsidize {
                settlement_id: SettlementId(2),
                gold: 50,
                resources: BTreeMap::from([
                    (crate::test_support::material("wood"), 20),
                    (crate::test_support::material("stone"), 5),
                ]),
            },
            NationAction::EnactDecree {
                decree: DecreeId::new("agricultural_decree").unwrap(),
            },
            NationAction::RevokeDecree {
                decree: DecreeId::new("games").unwrap(),
            },
        ]
    }

    #[test]
    fn every_action_survives_a_json_round_trip() {
        let actions = one_of_each();
        let kinds: Vec<_> = actions.iter().map(NationAction::kind).collect();
        assert_eq!(
            kinds,
            NationActionKind::ALL,
            "the list covers every variant once"
        );
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
                {"type": "revoke_decree", "decree": "games"}
            ]
        });
        // Compared as the text an agent receives: `to_value` would widen the f32
        // rate to f64 (0.15 becomes 0.15000000596…), the JSON text keeps 0.15.
        let text: serde_json::Value = serde_json::from_str(&decision.to_json()).unwrap();
        assert_eq!(text, expected);
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
            wrap(r#"{"type": "set_tax_rate", "rate": "0.1"}"#),
            wrap(r#"{"type": "set_tax_rate", "rate": null}"#),
            wrap(r#"{"type": "set_tax_rate"}"#),
            wrap(r#"{"type": "subsidize", "settlement_id": 1, "gold": -1}"#),
            wrap(r#"{"type": "subsidize", "settlement_id": 1, "resources": {"Gold": 1}}"#),
            wrap(r#"{"type": "subsidize", "settlement_id": 1, "army": 3}"#),
            wrap(r#"{"type": "declare_war", "target": 2}"#),
            wrap(r#"{"type": "enact_decree", "decree": "Free Beer"}"#),
            wrap(r#"{"type": "enact_decree"}"#),
            wrap(r#"{"type": "revoke_decree", "decree": 3}"#),
            wrap(r#"{"type": "relocate_capital", "settlement_id": 3}"#),
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
