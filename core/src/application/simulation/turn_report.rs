use crate::domain::ids::SettlementId;
use serde::Serialize;
use std::collections::BTreeMap;

use super::DecisionOutcome;
use crate::domain::settlement::turn_report::TurnReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PhaseExecution {
    pub phase: TurnPhase,
    pub settlement_id: SettlementId,
}

/// Complete turn result, serializable for experiment logging.
#[derive(Debug, Clone, Serialize)]
pub struct SimulationTurnReport {
    /// Number of the global turn just completed; the first report has turn 1.
    pub turn: u64,
    pub decisions: Vec<DecisionOutcome>,
    pub settlements: BTreeMap<SettlementId, TurnReport>,
    pub execution: Vec<PhaseExecution>,
}
