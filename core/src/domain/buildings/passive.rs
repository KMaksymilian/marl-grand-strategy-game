//! Passive effects and settlement-wide placement rules for a building blueprint.
use crate::domain::economy::material_units_from_display;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TradeLimitBonuses {
    pub caravan_slots: u32,
    pub offer_slots: u32,
    pub caravan_cargo: u32,
    pub open_offer_units: u32,
}

impl TradeLimitBonuses {
    pub(crate) fn into_material_units(self) -> Option<Self> {
        Some(Self {
            caravan_cargo: material_units_from_display(self.caravan_cargo)?,
            open_offer_units: material_units_from_display(self.open_offer_units)?,
            ..self
        })
    }

    pub(crate) fn saturating_add(self, other: Self) -> Self {
        Self {
            caravan_slots: self.caravan_slots.saturating_add(other.caravan_slots),
            offer_slots: self.offer_slots.saturating_add(other.offer_slots),
            caravan_cargo: self.caravan_cargo.saturating_add(other.caravan_cargo),
            open_offer_units: self.open_offer_units.saturating_add(other.open_offer_units),
        }
    }
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
        self.trade = self.trade.saturating_add(other.trade);
        self.happiness = self.happiness.saturating_add(other.happiness);
        self.defense_basis_points = self
            .defense_basis_points
            .saturating_add(other.defense_basis_points);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingPlacementLimit {
    pub group: String,
    pub maximum: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildingUpgradeRule {
    #[default]
    Unrestricted,
    ConstructionOnly,
    From(String),
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
