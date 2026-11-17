use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::taxation::NationTurnReport;
use serde::Serialize;
use std::collections::BTreeMap;

use super::{DecisionOutcome, NationDecisionOutcome};
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
    /// Production tax (§6), between production and storage.
    Taxation,
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
    /// Nation decisions, applied before settlement decisions; ascending nation ID.
    pub nation_decisions: Vec<NationDecisionOutcome>,
    /// What every nation collected this turn; the nation agent's report.
    pub nations: BTreeMap<NationId, NationTurnReport>,
    pub decisions: Vec<DecisionOutcome>,
    pub settlements: BTreeMap<SettlementId, TurnReport>,
    pub execution: Vec<PhaseExecution>,
}
