//! Outcomes of the settlement commands executed in the decision phase.
use crate::ids::SettlementId;
pub use crate::settlement::command::SettlementCommand;
pub use crate::world::CommandError as DecisionError;

#[derive(Debug, Clone)]
pub struct DecisionOutcome {
    pub settlement_id: SettlementId,
    pub decision: SettlementCommand,
    pub result: Result<(), DecisionError>,
}
