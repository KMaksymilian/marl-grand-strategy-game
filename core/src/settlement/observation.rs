//! What one settlement agent may see. Observations are owned, serializable copies
//! generated from the authoritative state (`Simulation::observe`); they hold no
//! references into the world, so other settlements' state cannot be reached.
//!
//! Visible: the settlement's own economy, its last turn, the tiles of its own
//! province, the public building catalog and the IDs of other settlements as trade
//! partners. Not visible: other settlements' stock, gold, buildings, projects,
//! offers, caravans and any tile outside the own province.
use crate::economy::ResourceType;
use crate::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use crate::settlement::decision::OfferSpec;
use crate::spatial::Position;
use crate::world::terrain::elevation::Elevation;
use crate::world::terrain::moisture::Moisture;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type ResourceAmounts = BTreeMap<ResourceType, u32>;
pub type ResourceTotals = BTreeMap<ResourceType, u64>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementObservation {
    /// Completed global turns when the observation was generated.
    pub turn: u64,
    pub settlement_id: SettlementId,
    pub province_id: usize,
    pub population: u32,
    pub gold: u32,
    pub inventory: InventoryObservation,
    pub workforce: WorkforceObservation,
    /// Materials the population requests every turn at its current size.
    pub needs_per_turn: ResourceAmounts,
    /// Ascending building ID.
    pub buildings: Vec<BuildingObservation>,
    /// Active projects in queue order.
    pub projects: Vec<ProjectObservation>,
    /// ID the next accepted construction or upgrade will receive.
    pub next_project_id: Option<ProjectId>,
    pub trade: TradeObservation,
    /// Summary of this settlement's last completed turn; `None` before the first.
    pub last_turn: Option<LastTurnObservation>,
    /// Every tile of the own province, in row-major order.
    pub territory: Vec<TileObservation>,
    /// Other settlements that caravans can be sent to, ascending ID.
    pub trade_partners: Vec<TradePartner>,
    /// Building types available for construction and upgrades, ascending ID.
    pub building_types: Vec<BuildingTypeObservation>,
}

impl SettlementObservation {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("observation values always serialize")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryObservation {
    pub capacity: u32,
    /// Warehouse materials (Labour excluded; it is reported as workforce).
    pub stored: ResourceAmounts,
    /// Sum of `stored`, counted against `capacity`.
    pub stored_total: u64,
    /// Production waiting in the buffer for the warehouse.
    pub pending: ResourceAmounts,
    pub storage_order: Vec<ResourceType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkforceObservation {
    /// Workers available each turn (the population).
    pub available: u32,
    /// Staff requested by all active buildings for the next turn.
    pub requested: u64,
    /// Staff the buildings actually received in the last allocation.
    pub employed: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingObservation {
    pub building_id: BuildingId,
    pub building_type: BuildingTypeId,
    pub position: Position,
    pub active: bool,
    pub max_workers: u32,
    /// Staff requested from the next allocation (zero when inactive).
    pub requested_workers: u32,
    /// Staff received in the last allocation.
    pub current_workers: u32,
    /// Output multiplier from travel distance, in 0..=1.
    pub location_productivity: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectObservation {
    pub project_id: ProjectId,
    pub target: ProjectTarget,
    /// Materials still to be paid.
    pub remaining_resources: ResourceAmounts,
    /// Turns still to elapse.
    pub remaining_turns: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectTarget {
    Construct {
        building_type: BuildingTypeId,
        position: Position,
    },
    Upgrade {
        building_id: BuildingId,
        building_type: BuildingTypeId,
    },
    /// A project kind agents cannot submit yet.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradeObservation {
    /// Published market offers, ascending resource.
    pub market_offers: Vec<OfferSpec>,
    /// Own caravans currently travelling or trading in the world.
    pub caravans_in_transit: usize,
    /// Own caravans back home, unloaded during the next turn.
    pub caravans_returning: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LastTurnObservation {
    /// Share of the population's needs met, in percent.
    pub satisfaction_percent: f32,
    pub missing_needs: Vec<ResourceType>,
    pub produced: ResourceTotals,
    pub population_consumed: ResourceTotals,
    pub project_consumed: ResourceTotals,
    pub production_consumed: ResourceTotals,
    /// Materials that did not fit the buffer or warehouse.
    pub lost: ResourceTotals,
    /// Materials moved from stock into the market and new caravans.
    pub trade_reserved: ResourceTotals,
    /// Materials back from the market and returning caravans.
    pub trade_received: ResourceTotals,
    pub gold_received: u64,
    /// Trades made at the own market with visiting caravans.
    pub market_trades: usize,
    pub completed_projects: Vec<ProjectId>,
    pub failed_projects: Vec<ProjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileObservation {
    pub position: Position,
    pub elevation: Elevation,
    pub moisture: Moisture,
    pub occupancy: TileState,
    /// A `Construct` action on this tile would pass placement checks.
    pub buildable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum TileState {
    Empty,
    OwnBuilding {
        building_id: BuildingId,
    },
    OwnConstruction {
        project_id: ProjectId,
    },
    /// A structure of another settlement, e.g. after a province border change.
    /// Its owner and details are hidden.
    Foreign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradePartner {
    pub settlement_id: SettlementId,
    pub province_id: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingTypeObservation {
    pub building_type: BuildingTypeId,
    pub name: String,
    pub tier: u8,
    pub max_workers: u32,
    pub construction_resources: ResourceAmounts,
    pub construction_turns: u32,
    /// Inputs and outputs per turn at full staff.
    pub recipes: Vec<RecipeObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeObservation {
    pub inputs: ResourceAmounts,
    pub outputs: ResourceAmounts,
}
