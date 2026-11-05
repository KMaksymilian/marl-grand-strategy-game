//! Serializable decisions submitted by rule-based or learning settlement agents.
//!
//! This is the data contract only. Values are checked against the world by the
//! action validator before execution; deserialization checks only the shape and
//! the numeric types (all quantities are unsigned, so negative values never parse).
//!
//! JSON shape, as sent from Python:
//! ```json
//! {
//!   "settlement_id": 1,
//!   "actions": [
//!     { "type": "construct", "building_type": "farm", "position": { "x": 3, "y": 4 } },
//!     { "type": "set_building_workers", "building_id": 2, "workers": 3 },
//!     { "type": "clear_market_offers" }
//!   ]
//! }
//! ```
use crate::economy::ResourceType;
use crate::economy::storage_order::StorageOrderError;
use crate::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use crate::spatial::Position;
use crate::trade::offer::TradeOffer;
use serde::{Deserialize, Serialize};
use std::fmt;

/// All actions one settlement submits for one turn.
///
/// Actions are applied in list order. Each one is accepted or rejected on its own:
/// a rejected action changes nothing and does not stop the actions after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementDecision {
    pub settlement_id: SettlementId,
    #[serde(default)]
    pub actions: Vec<SettlementAction>,
}

impl SettlementDecision {
    pub fn new(settlement_id: SettlementId, actions: Vec<SettlementAction>) -> Self {
        Self {
            settlement_id,
            actions,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("decision values always serialize")
    }

    pub fn from_json(source: &str) -> Result<Self, DecisionParseError> {
        serde_json::from_str(source).map_err(DecisionParseError)
    }
}

/// One settlement decision. Parameters reference world objects by ID; costs and
/// building definitions are always taken from the simulation, never from the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SettlementAction {
    /// Start building a catalog building on an empty land tile of the own province.
    /// The project ID is assigned by the simulation and the cost comes from the catalog.
    Construct {
        building_type: BuildingTypeId,
        position: Position,
    },
    /// Replace an own building's definition with another catalog building.
    Upgrade {
        building_id: BuildingId,
        building_type: BuildingTypeId,
    },
    /// Stop an own active project. Materials already paid are not refunded.
    CancelProject { project_id: ProjectId },
    /// Requested staff of an own building, in `0..=max_workers` of its definition.
    /// Zero switches the building off; recipe inputs scale with the staff.
    SetBuildingWorkers {
        building_id: BuildingId,
        workers: u32,
    },
    /// Warehouse admission order: every material exactly once, without Labour.
    SetStorageOrder { order: Vec<ResourceType> },
    /// Replace all published market offers. Offers stay published until replaced
    /// or cleared; submitting nothing keeps the previous offers.
    PublishMarketOffers { offers: Vec<OfferSpec> },
    /// Remove all market offers. Visiting caravans then find nothing to trade.
    ClearMarketOffers,
    /// Send a caravan with these offers to another settlement. Goods and gold are
    /// taken from stock when the caravan is funded during the turn.
    OrderCaravan {
        receiver_id: SettlementId,
        offers: Vec<OfferSpec>,
    },
}

impl SettlementAction {
    pub fn kind(&self) -> ActionKind {
        match self {
            Self::Construct { .. } => ActionKind::Construct,
            Self::Upgrade { .. } => ActionKind::Upgrade,
            Self::CancelProject { .. } => ActionKind::CancelProject,
            Self::SetBuildingWorkers { .. } => ActionKind::SetBuildingWorkers,
            Self::SetStorageOrder { .. } => ActionKind::SetStorageOrder,
            Self::PublishMarketOffers { .. } => ActionKind::PublishMarketOffers,
            Self::ClearMarketOffers => ActionKind::ClearMarketOffers,
            Self::OrderCaravan { .. } => ActionKind::OrderCaravan,
        }
    }
}

/// Action variant without parameters; used for action masks and reports.
/// Serialized names match the `type` tag of `SettlementAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Construct,
    Upgrade,
    CancelProject,
    SetBuildingWorkers,
    SetStorageOrder,
    PublishMarketOffers,
    ClearMarketOffers,
    OrderCaravan,
}

impl ActionKind {
    /// Every kind in a fixed order, e.g. for a fixed-size mask vector.
    pub const ALL: [Self; 8] = [
        Self::Construct,
        Self::Upgrade,
        Self::CancelProject,
        Self::SetBuildingWorkers,
        Self::SetStorageOrder,
        Self::PublishMarketOffers,
        Self::ClearMarketOffers,
        Self::OrderCaravan,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfferSide {
    Buy,
    Sell,
}

/// Parameters of one trade offer. Purchase progress is never part of the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferSpec {
    /// Any material except Labour; at most one offer per resource in a list.
    pub resource: ResourceType,
    pub side: OfferSide,
    /// Units to sell or buy; must be positive.
    pub units: u32,
    /// Gold per unit: the minimum when selling, the maximum when buying.
    pub price_per_unit: u32,
}

impl OfferSpec {
    pub fn to_offer(&self) -> TradeOffer {
        match self.side {
            OfferSide::Buy => TradeOffer::buy(self.units, self.price_per_unit),
            OfferSide::Sell => TradeOffer::sell(self.units, self.price_per_unit),
        }
    }
}

/// Why one action of a decision was rejected. A rejected action changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionError {
    UnknownBuildingType {
        building_type: BuildingTypeId,
    },
    /// No building with this ID belongs to the settlement.
    UnknownBuilding {
        building_id: BuildingId,
    },
    /// No active project with this ID belongs to the settlement.
    UnknownProject {
        project_id: ProjectId,
    },
    /// Caravan receiver is not a registered settlement.
    UnknownSettlement {
        settlement_id: SettlementId,
    },
    /// Outside the map.
    InvalidPosition {
        position: Position,
    },
    /// On the map but not in the settlement's own province.
    OutsideOwnProvince {
        position: Position,
    },
    OceanTile {
        position: Position,
    },
    /// A building or construction site already occupies the tile.
    TileOccupied {
        position: Position,
    },
    /// The building already has an active upgrade project.
    BuildingAlreadyUpgrading {
        building_id: BuildingId,
    },
    SameBuildingType {
        building_id: BuildingId,
    },
    WorkersAboveCapacity {
        building_id: BuildingId,
        workers: u32,
        max_workers: u32,
    },
    /// The requested staff of all buildings would exceed the population.
    InsufficientWorkers {
        requested: u64,
        available: u32,
    },
    /// Valid count that the building's load cannot express exactly.
    UnsupportedWorkerCount {
        building_id: BuildingId,
        workers: u32,
    },
    InvalidStorageOrder {
        error: StorageOrderError,
    },
    LabourIsNotTradable,
    ZeroUnits {
        resource: ResourceType,
    },
    DuplicateOffer {
        resource: ResourceType,
    },
    /// A caravan order without offers.
    EmptyOrder,
    TradeWithSelf,
    /// Caravans in this decision would sell more than the warehouse holds.
    InsufficientStock {
        resource: ResourceType,
        requested: u64,
        available: u32,
    },
    /// Caravans in this decision would need more gold than the treasury holds.
    InsufficientGold {
        requested: u64,
        available: u32,
    },
    ProjectIdsExhausted,
    /// An earlier action of the same decision already targets the same object.
    Conflict {
        with_index: usize,
    },
    /// The engine refused an action that passed validation. This indicates a
    /// mismatch between validator and engine rules; the state is unchanged.
    ExecutionFailed,
}

/// What an applied action changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionEffect {
    /// Construction or upgrade queued under a simulation-assigned ID.
    ProjectQueued {
        project_id: ProjectId,
    },
    ProjectCanceled {
        project_id: ProjectId,
    },
    /// The building is active and requests this staff from the next allocation on.
    WorkersSet {
        building_id: BuildingId,
        workers: u32,
    },
    StorageOrderSet,
    MarketOffersPublished,
    MarketOffersCleared,
    /// Funded from stock and gold during this turn, then sent to the receiver.
    CaravanOrdered {
        receiver_id: SettlementId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionOutcome {
    Applied { effect: ActionEffect },
    Rejected { error: ActionError },
}

/// Result of one submitted action, identified by its position in the decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionResult {
    pub index: usize,
    pub kind: ActionKind,
    pub outcome: ActionOutcome,
}

impl ActionResult {
    pub fn is_applied(&self) -> bool {
        matches!(self.outcome, ActionOutcome::Applied { .. })
    }
}

/// Results of one decision, one entry per action in submission order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionOutcome {
    pub settlement_id: SettlementId,
    pub results: Vec<ActionResult>,
}

/// A rejected action, identified by its position in `SettlementDecision::actions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RejectedAction {
    pub index: usize,
    pub kind: ActionKind,
    pub error: ActionError,
}

/// The JSON text is malformed or does not match the decision contract.
#[derive(Debug)]
pub struct DecisionParseError(serde_json::Error);

impl fmt::Display for DecisionParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid settlement decision: {}", self.0)
    }
}

impl std::error::Error for DecisionParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ResourceType::{Grain, Stone, Wood};

    fn offer(resource: ResourceType, side: OfferSide, units: u32, price: u32) -> OfferSpec {
        OfferSpec {
            resource,
            side,
            units,
            price_per_unit: price,
        }
    }

    fn one_of_each() -> Vec<SettlementAction> {
        vec![
            SettlementAction::Construct {
                building_type: BuildingTypeId::new("farm"),
                position: Position { x: 3, y: 4 },
            },
            SettlementAction::Upgrade {
                building_id: BuildingId(2),
                building_type: BuildingTypeId::new("big_farm"),
            },
            SettlementAction::CancelProject {
                project_id: ProjectId(5),
            },
            SettlementAction::SetBuildingWorkers {
                building_id: BuildingId(2),
                workers: 3,
            },
            SettlementAction::SetStorageOrder {
                order: vec![Grain, Wood, Stone],
            },
            SettlementAction::PublishMarketOffers {
                offers: vec![
                    offer(Wood, OfferSide::Sell, 10, 2),
                    offer(Grain, OfferSide::Buy, 4, 3),
                ],
            },
            SettlementAction::ClearMarketOffers,
            SettlementAction::OrderCaravan {
                receiver_id: SettlementId(2),
                offers: vec![offer(Stone, OfferSide::Sell, 6, 1)],
            },
        ]
    }

    #[test]
    fn every_action_survives_a_json_round_trip() {
        let actions = one_of_each();
        let kinds: Vec<_> = actions.iter().map(SettlementAction::kind).collect();
        assert_eq!(kinds, ActionKind::ALL, "the list covers every variant once");
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let back: SettlementAction = serde_json::from_str(&json).unwrap();
            assert_eq!(back, action, "{json}");
        }
    }

    #[test]
    fn decision_survives_a_json_round_trip() {
        let decision = SettlementDecision::new(SettlementId(1), one_of_each());
        let back = SettlementDecision::from_json(&decision.to_json()).unwrap();
        assert_eq!(back, decision);
    }

    #[test]
    fn json_shape_is_flat_and_uses_snake_case_tags() {
        let decision = SettlementDecision::new(
            SettlementId(1),
            vec![
                SettlementAction::Construct {
                    building_type: BuildingTypeId::new("farm"),
                    position: Position { x: 3, y: -4 },
                },
                SettlementAction::ClearMarketOffers,
                SettlementAction::OrderCaravan {
                    receiver_id: SettlementId(2),
                    offers: vec![offer(Wood, OfferSide::Buy, 5, 7)],
                },
            ],
        );
        let expected = serde_json::json!({
            "settlement_id": 1,
            "actions": [
                {"type": "construct", "building_type": "farm", "position": {"x": 3, "y": -4}},
                {"type": "clear_market_offers"},
                {"type": "order_caravan", "receiver_id": 2, "offers": [
                    {"resource": "Wood", "side": "buy", "units": 5, "price_per_unit": 7}
                ]}
            ]
        });
        assert_eq!(serde_json::to_value(&decision).unwrap(), expected);
    }

    #[test]
    fn hand_written_json_from_an_agent_is_accepted() {
        let source = r#"{
            "settlement_id": 7,
            "actions": [
                {"type": "set_building_workers", "building_id": 1, "workers": 0},
                {"type": "set_storage_order", "order": ["Stone", "Wood", "Grain"]},
                {"type": "cancel_project", "project_id": 4}
            ]
        }"#;
        let decision = SettlementDecision::from_json(source).unwrap();
        assert_eq!(decision.settlement_id, SettlementId(7));
        assert_eq!(
            decision.actions,
            vec![
                SettlementAction::SetBuildingWorkers {
                    building_id: BuildingId(1),
                    workers: 0
                },
                SettlementAction::SetStorageOrder {
                    order: vec![Stone, Wood, Grain]
                },
                SettlementAction::CancelProject {
                    project_id: ProjectId(4)
                },
            ]
        );
    }

    #[test]
    fn missing_action_list_means_no_actions() {
        let decision = SettlementDecision::from_json(r#"{"settlement_id": 3}"#).unwrap();
        assert_eq!(decision, SettlementDecision::new(SettlementId(3), vec![]));
    }

    #[test]
    fn malformed_decisions_are_rejected_when_parsing() {
        let wrap = |action: &str| format!(r#"{{"settlement_id": 1, "actions": [{action}]}}"#);
        let cases = [
            wrap(r#"{"type": "set_building_workers", "building_id": 1, "workers": -1}"#),
            wrap(r#"{"type": "set_building_workers", "building_id": 1, "workers": 1.5}"#),
            wrap(r#"{"type": "set_building_workers", "building_id": -2, "workers": 1}"#),
            wrap(r#"{"type": "set_building_workers", "building_id": 1}"#),
            wrap(r#"{"type": "set_building_workers", "building_id": 1, "workers": 1, "load": 1}"#),
            wrap(r#"{"type": "recruit_army"}"#),
            wrap(r#"{"building_id": 1}"#),
            wrap(r#"{"type": "set_storage_order", "order": ["Gold"]}"#),
            wrap(r#"{"type": "construct", "building_type": "farm", "position": {"x": 1}}"#),
            wrap(
                r#"{"type": "publish_market_offers", "offers": [{"resource": "Wood", "side": "sell", "units": -3, "price_per_unit": 1}]}"#,
            ),
            wrap(
                r#"{"type": "publish_market_offers", "offers": [{"resource": "Wood", "side": "lend", "units": 3, "price_per_unit": 1}]}"#,
            ),
            r#"{"settlement_id": -1, "actions": []}"#.to_owned(),
            r#"{"settlement_id": 1, "actions": [], "turn": 3}"#.to_owned(),
            r#"{"actions": []}"#.to_owned(),
            "not json".to_owned(),
        ];
        for source in cases {
            let error = SettlementDecision::from_json(&source);
            assert!(error.is_err(), "accepted: {source}");
            assert!(
                error
                    .unwrap_err()
                    .to_string()
                    .starts_with("invalid settlement decision")
            );
        }
    }

    #[test]
    fn action_kinds_serialize_like_action_tags() {
        for (action, kind) in one_of_each().iter().zip(ActionKind::ALL) {
            let tag = serde_json::to_value(action).unwrap()["type"].clone();
            assert_eq!(tag, serde_json::to_value(kind).unwrap());
        }
    }

    #[test]
    fn offer_specs_become_fresh_trade_offers() {
        assert_eq!(
            offer(Wood, OfferSide::Sell, 8, 2).to_offer(),
            TradeOffer::sell(8, 2)
        );
        assert_eq!(
            offer(Grain, OfferSide::Buy, 4, 3).to_offer(),
            TradeOffer::buy(4, 3)
        );
    }
}
