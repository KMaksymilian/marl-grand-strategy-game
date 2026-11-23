//! Passive effects and settlement-wide placement rules for a building blueprint.
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TradeLimitBonuses {
    pub caravan_slots: u32,
    pub offer_slots: u32,
    pub caravan_cargo: u32,
    pub open_offer_units: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PassiveBuildingEffects {
    pub warehouse_capacity: u32,
    pub trade: TradeLimitBonuses,
    pub happiness: u32,
    /// Added to the defender's multiplier. 2,500 basis points means +25%.
    pub defense_basis_points: u32,
}

impl PassiveBuildingEffects {
    pub(crate) fn add(&mut self, other: Self) {
        self.warehouse_capacity = self
            .warehouse_capacity
            .saturating_add(other.warehouse_capacity);
        self.trade.caravan_slots = self
            .trade
            .caravan_slots
            .saturating_add(other.trade.caravan_slots);
        self.trade.offer_slots = self
            .trade
            .offer_slots
            .saturating_add(other.trade.offer_slots);
        self.trade.caravan_cargo = self
            .trade
            .caravan_cargo
            .saturating_add(other.trade.caravan_cargo);
        self.trade.open_offer_units = self
            .trade
            .open_offer_units
            .saturating_add(other.trade.open_offer_units);
        self.happiness = self.happiness.saturating_add(other.happiness);
        self.defense_basis_points = self
            .defense_basis_points
            .saturating_add(other.defense_basis_points);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingPlacementLimit {
    pub group: String,
    pub maximum: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildingUpgradeRule {
    #[default]
    Unrestricted,
    ConstructionOnly,
    From(String),
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PassiveBuildingRules {
    pub effects: PassiveBuildingEffects,
    pub limit: Option<BuildingPlacementLimit>,
    pub upgrade: BuildingUpgradeRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildingRuleError {
    LimitReached { group: String, maximum: u32 },
    ConstructionOnly { building_type: String },
    UpgradeRequires { previous_type: String },
}
