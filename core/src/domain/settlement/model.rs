//! Settlement state, read-only views and construction.
//! Commands and turn processing are implemented on the same type in child modules.
mod commands;
mod economy;
pub mod projects;
mod trading;
mod turn;

pub use commands::TradeCommandError;
pub use economy::{NeedRegistry, NeedTier};
pub use trading::CaravanOrder;
pub(crate) use turn::PreparedTurn;
pub use turn::report as turn_report;

use crate::domain::buildings::Building;
use crate::domain::buildings::productivity::BuildingProductivityCalculator;
use crate::domain::economy::ResourceType;
use crate::domain::economy::gold_inventory::GoldInventory;
use crate::domain::ids::{BuildingId, SettlementId};
use crate::domain::spatial::Position;
use crate::domain::trade::book::TradeBook;
use economy::{BuildingManager, PopulationManager, SettlementInventoryManager};
use projects::Project;
use projects::ProjectManager;
use std::sync::Arc;
use trading::TradeManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingCommandError {
    DuplicateBuilding(BuildingId),
    Territory(crate::domain::spatial::TerritoryError),
    MissingBuilding(BuildingId),
    InvalidLoad,
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
        Self {
            id,
            capital: None,
            gold_inventory: GoldInventory::default(),
            inventory_manager: SettlementInventoryManager::new(initial_capacity),
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
    pub(crate) fn assign_capital(&mut self, capital: Position) {
        self.capital = Some(capital);
        self.building_manager.use_capital(capital);
    }

    pub fn population(&self) -> u32 {
        self.population_manager.population
    }

    /// Materials the population requests every turn at its current size.
    pub fn needs_per_turn(&self) -> std::collections::HashMap<ResourceType, u32> {
        self.population_manager.calculate_demand()
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
