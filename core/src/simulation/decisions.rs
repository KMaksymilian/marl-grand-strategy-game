//! Compatibility names for domain commands and world command outcomes.
use crate::ids::SettlementId;
pub use crate::settlement::command::SettlementCommand as SettlementDecision;
pub use crate::world::CommandError as DecisionError;

#[derive(Debug, Clone)]
pub struct DecisionOutcome {
    pub settlement_id: SettlementId,
    pub decision: SettlementDecision,
    pub result: Result<(), DecisionError>,
}
