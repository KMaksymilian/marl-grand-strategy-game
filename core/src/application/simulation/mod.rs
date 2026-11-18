//! Headless global turn coordination. World remains the only settlement registry.
use crate::domain::ids::SettlementId;
mod turn_coordinator;
pub mod turn_report;

pub use crate::application::commands::nation::contract::{
    NationAction, NationActionEffect, NationActionError, NationActionKind, NationActionOutcome,
    NationActionResult, NationDecision, NationDecisionOutcome,
};
pub use crate::application::commands::settlement::contract::{
    ActionEffect, ActionError, ActionOutcome, ActionResult, DecisionOutcome, SettlementAction,
    SettlementDecision,
};
pub use crate::application::queries::nation_observation::{
    NationObservation, OwnedSettlementObservation,
};
pub use crate::domain::nation::report::NationTurnReport;
pub use crate::domain::world::OwnershipChange;
pub use turn_report::{PhaseExecution, SimulationTurnReport, TurnPhase};

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use crate::application::queries::{
    observation::SettlementObservation, observe_nation, observe_settlement,
};
use crate::domain::ids::NationId;
use crate::domain::settlement::turn_report::TurnReport;
use crate::domain::settlement::{NeedRegistry, Settlement};
use crate::domain::world::settlement_registry::SettlementRegistry;
use crate::domain::world::{Position, World, WorldError, pathfinding::path_finder::PathFinder};
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
    finder: PathFinder,
    pending_decisions: BTreeMap<SettlementId, Vec<SettlementDecision>>,
    /// Nation decisions waiting for the next `step`; they run before settlement
    /// decisions. Kept apart because the two agent kinds have different contracts.
    pending_nation_decisions: BTreeMap<NationId, Vec<NationDecision>>,
    /// Each settlement's report from the latest completed turn, for observations.
    last_reports: BTreeMap<SettlementId, TurnReport>,
    /// Each nation's report from the latest completed turn, for observations.
    last_nation_reports: BTreeMap<NationId, NationTurnReport>,
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

    pub fn initialize_settlement_at(
        &mut self,
        province_id: usize,
        capital: Position,
        initial_capacity: u32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
    ) -> Result<SettlementId, WorldError> {
        self.world.initialize_settlement_at(
            province_id,
            capital,
            initial_capacity,
            initial_population,
            need_registry,
        )
    }

    /// Takes a configured world. Turn zero means no global turns have completed.
    pub fn new(world: World) -> Result<Self, SimulationError> {
        let map = world.map();
        let area = map
            .width
            .checked_mul(map.height)
            .ok_or(SimulationError::InvalidMap)?;
        let provinces: HashSet<_> = world
            .provinces()
            .iter()
            .map(|province| province.id)
            .collect();
        if area == 0
            || map.terrain.len() != area
            || map.territory.len() != area
            || provinces.len() != world.provinces().len()
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
            finder,
            pending_decisions: BTreeMap::new(),
            pending_nation_decisions: BTreeMap::new(),
            last_reports: BTreeMap::new(),
            last_nation_reports: BTreeMap::new(),
        })
    }

    /// Advances one global turn and returns the outcomes of submitted decisions.
    pub fn step(&mut self) -> Result<SimulationTurnReport, SimulationError> {
        let current_turn = self.current_turn();
        let report = turn_coordinator::run_turn(
            &mut self.world,
            &mut self.finder,
            &mut self.pending_decisions,
            &mut self.pending_nation_decisions,
            current_turn,
        )?;
        self.world.set_completed_turn(report.turn);
        self.last_reports = report.settlements.clone();
        self.last_nation_reports = report.nations.clone();
        Ok(report)
    }

    pub fn current_turn(&self) -> u64 {
        self.world.completed_turn()
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

    pub fn add_settlement_at(
        &mut self,
        province_id: usize,
        capital: Position,
        settlement: Settlement,
    ) -> Result<(), WorldError> {
        self.world
            .add_settlement_at(province_id, capital, settlement)
    }

    /// Queues an agent decision for the next successful step; does not change world
    /// state. Several decisions of one settlement run in submission order, each
    /// validated against the state left by the previous one.
    pub fn queue_decision(&mut self, decision: SettlementDecision) -> Result<(), WorldError> {
        let settlement_id = decision.settlement_id;
        if self.world.settlement(settlement_id).is_none() {
            return Err(WorldError::MissingSettlement(settlement_id));
        }
        self.pending_decisions
            .entry(settlement_id)
            .or_default()
            .push(decision);
        Ok(())
    }

    /// What the settlement's agent may see now: its own state, last turn and
    /// province, never other settlements' internals.
    pub fn observe(
        &self,
        settlement_id: SettlementId,
    ) -> Result<SettlementObservation, WorldError> {
        observe_settlement::observe_settlement(
            &self.world,
            settlement_id,
            self.current_turn(),
            self.last_reports.get(&settlement_id),
        )
    }

    /// Observations of every settlement, ascending ID.
    pub fn observe_all(&self) -> BTreeMap<SettlementId, SettlementObservation> {
        self.world
            .settlements()
            .map(|settlement| {
                let id = settlement.id();
                (id, self.observe(id).expect("registered settlement"))
            })
            .collect()
    }

    /// Registers an empty settlement as a member of an existing nation.
    pub fn add_settlement_to_nation(
        &mut self,
        nation_id: NationId,
        province_id: usize,
        settlement: Settlement,
    ) -> Result<(), WorldError> {
        self.world
            .add_settlement_to_nation(nation_id, province_id, settlement)
    }

    /// Queues a nation decision for the next successful step. Nation decisions
    /// run before settlement decisions; several decisions of one nation run in
    /// submission order.
    pub fn queue_nation_decision(&mut self, decision: NationDecision) -> Result<(), WorldError> {
        let nation_id = decision.nation_id;
        if self.world.nation(nation_id).is_none() {
            return Err(WorldError::MissingNation(nation_id));
        }
        self.pending_nation_decisions
            .entry(nation_id)
            .or_default()
            .push(decision);
        Ok(())
    }

    /// Marks whether an enemy army stands in the settlement's province; an
    /// attacked settlement pays no tax (see `World::set_settlement_under_attack`).
    pub fn set_settlement_under_attack(
        &mut self,
        settlement_id: SettlementId,
        attacked: bool,
    ) -> Result<(), WorldError> {
        self.world
            .set_settlement_under_attack(settlement_id, attacked)
    }

    /// Gives a settlement to another nation (see `World::transfer_settlement`).
    /// A nation that goes extinct loses its pending decisions and last report.
    pub fn transfer_settlement(
        &mut self,
        settlement_id: SettlementId,
        to: NationId,
    ) -> Result<OwnershipChange, WorldError> {
        let change = self.world.transfer_settlement(settlement_id, to)?;
        if change.extinct {
            self.pending_nation_decisions.remove(&change.from);
            self.last_nation_reports.remove(&change.from);
        }
        Ok(change)
    }

    /// What the nation's agent may see now.
    pub fn observe_nation(&self, nation_id: NationId) -> Result<NationObservation, WorldError> {
        observe_nation::observe_nation(
            &self.world,
            nation_id,
            self.current_turn(),
            self.last_nation_reports.get(&nation_id),
        )
    }

    /// Observations of every nation, ascending ID.
    pub fn observe_all_nations(&self) -> BTreeMap<NationId, NationObservation> {
        self.world
            .nations()
            .iter()
            .map(|nation| {
                let id = nation.id();
                (id, self.observe_nation(id).expect("registered nation"))
            })
            .collect()
    }

    /// Recovers the world, e.g. to edit a scenario. Pending decisions are discarded.
    pub fn into_world(self) -> World {
        self.world
    }
}
