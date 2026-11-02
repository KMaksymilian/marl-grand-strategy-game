//! Settlement state, read-only views and construction.
//! Commands and turn processing are implemented on the same type in child modules.
mod building_manager;
mod commands;
mod inventory_manager;
mod population;
mod project_lifecycle;
mod project_manager;
pub mod projects;
pub mod stat;
mod trade_lifecycle;
mod trade_manager;
mod turn;
pub mod turn_report;

pub use commands::TradeCommandError;
pub use trade_manager::CaravanOrder;

pub use population::{NeedRegistry, NeedTier};

use crate::buildings::Building;
use crate::buildings::productivity::BuildingProductivityCalculator;
use crate::economy::ResourceType;
use crate::economy::gold_inventory::GoldInventory;
use crate::ids::BuildingId;
use crate::trade::book::TradeBook;
use building_manager::BuildingManager;
use inventory_manager::SettlementInventoryManager;
use population::PopulationManager;
use project_manager::ProjectManager;
use projects::Project;
use std::sync::Arc;
use trade_manager::TradeManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingCommandError {
    MissingBuilding(BuildingId),
    InvalidLoad,
}

/// Managers are implementation details; public views cannot mutate their collections.
/// ```compile_fail
/// use simulation_core::settlement::Settlement;
/// let mut settlement = Settlement::new(1, 10, 1, std::sync::Arc::default());
/// settlement.project_manager.active_projects.clear();
/// ```
#[derive(Debug)]
pub struct Settlement {
    id: u32,
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
        id: u32,
        initial_capacity: u32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
    ) -> Self {
        Self {
            id,
            gold_inventory: GoldInventory::default(),
            inventory_manager: SettlementInventoryManager::new(initial_capacity),
            book: TradeBook::default(),
            trade: TradeManager::default(),
            building_manager: BuildingManager::new(),
            population_manager: PopulationManager::new(initial_population, need_registry),
            project_manager: ProjectManager::new(),
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn population(&self) -> u32 {
        self.population_manager.population
    }

    pub fn inventory(&self) -> &crate::economy::ResourceInventory {
        &self.inventory_manager.main
    }

    pub fn pending_inventory(&self) -> &crate::economy::ResourceInventory {
        &self.inventory_manager.pending_production
    }

    pub fn buildings(&self) -> impl Iterator<Item = &Building> {
        self.building_manager.buildings.iter()
    }

    pub fn active_projects(&self) -> &[Project] {
        &self.project_manager.active_projects
    }

    pub fn building_productivity_calculator(&self) -> Option<BuildingProductivityCalculator> {
        self.building_manager.productivity_calculator()
    }

    pub fn storage_order(&self) -> &[ResourceType] {
        self.inventory_manager.storage_order()
    }
}
