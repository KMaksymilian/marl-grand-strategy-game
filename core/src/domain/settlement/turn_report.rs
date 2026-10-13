pub use crate::domain::settlement::demographics::ConsumptionResult;
use crate::domain::settlement::projects::ProjectOutcome;

/// Results of one settlement turn. Pending projects produce no outcome entry.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnReport {
    pub consumption: ConsumptionResult,
    pub projects: Vec<ProjectOutcome>,
}
