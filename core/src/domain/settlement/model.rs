//! Settlement state, read-only views and construction.
//! Commands and turn processing are implemented on the same type in child modules.
mod commands;
mod economy;
mod passive_buildings;
pub mod projects;
mod trading;
mod turn;

pub use commands::TradeCommandError;
pub(crate) use commands::{CaravanUsage, offer_units};
pub use economy::{NeedRegistry, NeedTier, NeedsConfigError};
pub use trading::CaravanOrder;
pub use turn::report as turn_report;
pub(crate) use turn::{PreparedTurn, ProducedTurn};

use crate::domain::buildings::Building;
use crate::domain::buildings::productivity::BuildingProductivityCalculator;
use crate::domain::economy::gold_inventory::GoldInventory;
use crate::domain::economy::{ResourceCatalog, ResourceType};
use crate::domain::ids::{BuildingId, SettlementId};
use crate::domain::spatial::Position;
use crate::domain::trade::book::TradeBook;
use economy::{BuildingManager, PopulationManager, SettlementInventoryManager};
use projects::Project;
use projects::ProjectManager;
use std::sync::Arc;
use trading::TradeManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingCommandError {
    UnknownMaterial(ResourceType),
    DuplicateBuilding(BuildingId),
    Territory(crate::domain::spatial::TerritoryError),
    MissingBuilding(BuildingId),
    InvalidLoad,
    BuildingRule(crate::domain::buildings::passive::BuildingRuleError),
}

/// Managers are implementation details; public views cannot mutate their collections.
/// ```compile_fail
/// use simulation_core::settlement::Settlement;
/// let mut settlement = Settlement::new(simulation_core::ids::SettlementId(1), 10, 1, std::sync::Arc::default());
/// settlement.project_manager.active_projects.clear();
/// ```
#[derive(Debug)]
pub struct Settlement {
    id: SettlementId,
    capital: Option<Position>,
    gold_inventory: GoldInventory,
    inventory_manager: SettlementInventoryManager,
    book: TradeBook,
    trade: TradeManager,
    building_manager: BuildingManager,
    population_manager: PopulationManager,
    project_manager: ProjectManager,
}

impl Settlement {
    pub fn new(
        id: SettlementId,
        initial_capacity: u32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
    ) -> Self {
        Self::with_resource_catalog(
            id,
            initial_capacity,
            initial_population,
            need_registry,
            ResourceCatalog::standard(),
        )
    }

    pub fn with_resource_catalog(
        id: SettlementId,
        initial_capacity: u32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
        resources: &ResourceCatalog,
    ) -> Self {
        Self {
            id,
            capital: None,
            gold_inventory: GoldInventory::default(),
            inventory_manager: SettlementInventoryManager::new(initial_capacity, resources),
            book: TradeBook::default(),
            trade: TradeManager::default(),
            building_manager: BuildingManager::new(),
            population_manager: PopulationManager::new(initial_population, need_registry),
            project_manager: ProjectManager::new(),
        }
    }

    pub fn id(&self) -> SettlementId {
        self.id
    }

    /// The map position assigned by World at registration.
    pub fn capital(&self) -> Option<Position> {
        self.capital
    }

    /// Registration accepts empty settlements, so no building cache needs updating.
    pub(crate) fn assign_capital(
        &mut self,
        capital: Position,
        default_policy: Option<crate::domain::buildings::productivity::TravelCostEfficiencyPolicy>,
    ) {
        self.capital = Some(capital);
        self.building_manager.use_capital(capital, default_policy);
    }

    pub fn population(&self) -> u32 {
        self.population_manager.population
    }

    /// Materials the population requests every turn at its current size.
    pub fn needs_per_turn(&self) -> std::collections::HashMap<ResourceType, u32> {
        self.population_manager.calculate_demand()
    }

    pub(crate) fn needs_with_food_multiplier(
        &self,
        multiplier: u32,
    ) -> std::collections::HashMap<ResourceType, u32> {
        self.population_manager.plan(multiplier).demand()
    }

    pub(crate) fn first_unknown_population_material(&self) -> Option<ResourceType> {
        self.population_manager
            .registry
            .materials()
            .filter(|resource| !self.knows_material(resource))
            .min()
            .cloned()
    }

    pub fn inventory(&self) -> &crate::domain::economy::ResourceInventory {
        &self.inventory_manager.main
    }

    pub fn pending_inventory(&self) -> &crate::domain::economy::ResourceInventory {
        &self.inventory_manager.pending_production
    }

    pub fn buildings(&self) -> impl Iterator<Item = &Building> {
        self.building_manager.buildings.iter()
    }

    pub fn active_projects(&self) -> &[Project] {
        &self.project_manager.active_projects
    }

    /// ID the simulation gives to this settlement's next project.
    /// It is above every ID used before, including completed and canceled projects.
    pub fn next_project_id(&self) -> Option<crate::domain::ids::ProjectId> {
        self.project_manager.next_id()
    }

    pub fn building_productivity_calculator(&self) -> Option<BuildingProductivityCalculator> {
        self.building_manager.productivity_calculator()
    }

    pub fn storage_order(&self) -> &[ResourceType] {
        self.inventory_manager.storage_order()
    }
}
