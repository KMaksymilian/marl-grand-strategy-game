use crate::ids::SettlementId;
use std::collections::BTreeMap;

use super::DecisionOutcome;
use crate::settlement::turn_report::TurnReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TurnPhase {
    Decisions,
    Preparation,
    Allocation,
    Consumption,
    Construction,
    Production,
    EconomicUpdates,
}

/// Recorded at each actual phase invocation, in execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseExecution {
    pub phase: TurnPhase,
    pub settlement_id: SettlementId,
}

#[derive(Debug, Clone)]
pub struct SimulationTurnReport {
    /// Number of the global turn just completed; the first report has turn 1.
    pub turn: u64,
    pub decisions: Vec<DecisionOutcome>,
    pub settlements: BTreeMap<SettlementId, TurnReport>,
    pub execution: Vec<PhaseExecution>,
}
