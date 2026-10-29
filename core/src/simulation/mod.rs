//! Headless global turn coordination. World remains the only settlement registry.
use crate::ids::SettlementId;
mod decisions;
mod turn;
pub mod turn_report;

pub use decisions::{DecisionError, DecisionOutcome, SettlementDecision};
pub use turn_report::{PhaseExecution, SimulationTurnReport, TurnPhase};

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use crate::settlement::{NeedRegistry, Settlement, registry::SettlementRegistry};
use crate::world::{World, WorldError, pathfinding::path_finder::PathFinder};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationError {
    InvalidMap,
    TurnOverflow,
    World {
        settlement_id: SettlementId,
        error: WorldError,
    },
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMap => formatter.write_str(
                "simulation requires a valid rectangular map with unique, existing province IDs",
            ),
            Self::TurnOverflow => formatter.write_str("global turn counter is exhausted"),
            Self::World {
                settlement_id,
                error,
            } => write!(
                formatter,
                "settlement {settlement_id} cannot advance: {error:?}"
            ),
        }
    }
}

impl std::error::Error for SimulationError {}

pub struct Simulation {
    world: World,
    current_turn: u64,
    finder: PathFinder,
    pending_decisions: BTreeMap<SettlementId, Vec<SettlementDecision>>,
}

impl Simulation {
    /// The same ordered registry used by World and every global phase.
    pub fn settlement_registry(&self) -> &SettlementRegistry {
        self.world.settlement_registry()
    }

    pub fn settlement(&self, id: SettlementId) -> Result<&Settlement, WorldError> {
        self.world
            .settlement(id)
            .ok_or(WorldError::MissingSettlement(id))
    }

    pub fn initialize_settlement(
        &mut self,
        province_id: usize,
        initial_capacity: u32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
    ) -> Result<SettlementId, WorldError> {
        self.world.initialize_settlement(
            province_id,
            initial_capacity,
            initial_population,
            need_registry,
        )
    }

    /// Takes a configured world. Turn zero means no global turns have completed.
    pub fn new(world: World) -> Result<Self, SimulationError> {
        let map = &world.map;
        let area = map
            .width
            .checked_mul(map.height)
            .ok_or(SimulationError::InvalidMap)?;
        let provinces: HashSet<_> = world.provinces.iter().map(|province| province.id).collect();
        if area == 0
            || map.terrain.len() != area
            || map.territory.len() != area
            || provinces.len() != world.provinces.len()
            || map.territory.iter().any(|id| !provinces.contains(id))
            || map
                .terrain
                .iter()
                .any(|area| !area.elevation_val.is_finite() || !area.moisture_val.is_finite())
        {
            return Err(SimulationError::InvalidMap);
        }
        let finder = PathFinder::new(map.width, map.height);
        Ok(Self {
            world,
            current_turn: 0,
            finder,
            pending_decisions: BTreeMap::new(),
        })
    }

    pub fn current_turn(&self) -> u64 {
        self.current_turn
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn settlements(&self) -> impl Iterator<Item = &Settlement> {
        self.world.settlements()
    }

    /// Registration uses World's existing registry, never a parallel collection.
    pub fn add_settlement(
        &mut self,
        province_id: usize,
        settlement: Settlement,
    ) -> Result<(), WorldError> {
        self.world.add_settlement(province_id, settlement)
    }

    /// Queues a command for the next successful step; does not change world state.
    pub fn queue_decision(
        &mut self,
        settlement_id: SettlementId,
        decision: SettlementDecision,
    ) -> Result<(), WorldError> {
        if self.world.settlement(settlement_id).is_none() {
            return Err(WorldError::MissingSettlement(settlement_id));
        }
        self.pending_decisions
            .entry(settlement_id)
            .or_default()
            .push(decision);
        Ok(())
    }

    /// Recovers the world, e.g. to edit a scenario. Pending decisions are discarded.
    pub fn into_world(self) -> World {
        self.world
    }
}
