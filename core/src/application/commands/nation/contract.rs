//! Serializable decisions submitted by nation agents (gameplay specification §5.2).
//!
//! The contract holds the policy and orders implemented so far; decrees,
//! diplomacy, recruitment and army orders are added as new action variants.
//! Values are checked against the world before execution; deserialization checks
//! only the shape and the numeric types.
//!
//! JSON shape:
//! ```json
//! {
//!   "nation_id": 0,
//!   "actions": [
//!     { "type": "set_tax_rate", "rate": 0.15 },
//!     { "type": "subsidize", "settlement_id": 2, "gold": 50, "resources": { "Wood": 20 } }
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
    /// The common tax rate of all own settlements, as a fraction:
    /// 0.0 is 0%, 0.1 is 10%, 1.0 is 100%.
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
}

impl NationAction {
    pub fn kind(&self) -> NationActionKind {
        match self {
            Self::SetTaxRate { .. } => NationActionKind::SetTaxRate,
            Self::Subsidize { .. } => NationActionKind::Subsidize,
        }
    }
}

/// Action variant without parameters. Serialized names match the `type` tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NationActionKind {
    SetTaxRate,
    Subsidize,
}

impl NationActionKind {
    /// Every kind in a fixed order, e.g. for a fixed-size mask vector.
    pub const ALL: [Self; 2] = [Self::SetTaxRate, Self::Subsidize];
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
                 "resources": {"Stone": 5, "Wood": 20}}
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
            wrap(r#"{"type": "declare_war", "target": 2}"#),
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
