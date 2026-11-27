//! What one settlement agent may see. Observations are owned, serializable copies
//! generated from the authoritative state (`Simulation::observe`); they hold no
//! references into the world, so other settlements' state cannot be reached.
//!
//! Visible: the settlement's own economy, its last turn, the tiles of its own
//! province, the public building catalog and the IDs of other settlements as trade
//! partners. Not visible: other settlements' stock, gold, buildings, projects,
//! live offers, caravans and any tile outside the own province. Returned
//! caravans supply dated foreign-market snapshots.
pub use super::troop_observation::TroopObservation;
use crate::application::commands::settlement::contract::{OfferSide, OfferSpec};
use crate::domain::economy::ResourceType;
use crate::domain::ids::{BuildingId, BuildingTypeId, NationId, ProjectId, SettlementId};
use crate::domain::nation::decree::DecreeId;
use crate::domain::spatial::Position;
use crate::domain::world::terrain::elevation::Elevation;
use crate::domain::world::terrain::moisture::Moisture;
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
    pub gold: u64,
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
    /// Policy of the owning nation; every settlement belongs to one (§5.3).
    pub nation: NationPolicyObservation,
    /// Latest troop observation of each province from this settlement's own
    /// watch and its returned caravans, ascending province ID (MGS-45).
    pub troops: Vec<TroopObservation>,
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
    /// Latest delivered snapshot per foreign market, ascending market settlement ID.
    #[serde(default)]
    pub known_markets: Vec<KnownMarketObservation>,
    /// Own caravans currently travelling or trading in the world.
    pub caravans_in_transit: usize,
    /// Own caravans back home, unloaded during the next turn.
    pub caravans_returning: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownMarketObservation {
    pub market: SettlementId,
    pub province_id: usize,
    pub observed_turn: u64,
    pub offers: BTreeMap<ResourceType, KnownMarketOffer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownMarketOffer {
    pub side: OfferSide,
    /// Buy quantities are estimated separately at each price limit against the shared budget.
    pub units: u32,
    pub unit_price_limit: u32,
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
    /// Caravans funded and sent out.
    pub caravans_dispatched: usize,
    /// Caravan orders skipped because nothing could be funded, e.g. when the
    /// market, which is funded first, took the stock.
    pub unfunded_caravan_orders: usize,
    pub completed_projects: Vec<ProjectId>,
    pub failed_projects: Vec<ProjectId>,
    /// Materials the owning nation took from last turn's production (§6).
    pub production_tax: ResourceTotals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileObservation {
    pub position: Position,
    pub elevation: Elevation,
    pub moisture: Moisture,
    #[serde(default)]
    pub coastal: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deposit: Option<ResourceType>,
    pub occupancy: TileState,
    /// Basic ownership, terrain and vacancy checks. Check the blueprint's deposit separately.
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
    /// Owning nation (public information).
    pub nation_id: NationId,
    /// False while the two nations are at war or under an embargo (§8.1):
    /// caravans to this partner are rejected.
    pub trade_allowed: bool,
}

/// The owning nation's policy as seen by one of its settlements (§5.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NationPolicyObservation {
    pub nation_id: NationId,
    pub tax_rate: f32,
    pub capital: SettlementId,
    /// This settlement is the national capital.
    pub is_capital: bool,
    /// Decrees of the nation in effect this turn, ascending ID.
    pub active_decrees: Vec<DecreeId>,
    /// Nations this settlement's nation is at war with, ascending ID.
    pub at_war_with: Vec<NationId>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_deposit: Option<ResourceType>,
    #[serde(default)]
    pub requires_coast: bool,
    /// Inputs and outputs per turn at full staff.
    pub recipes: Vec<RecipeObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeObservation {
    pub inputs: ResourceAmounts,
    pub outputs: ResourceAmounts,
}
