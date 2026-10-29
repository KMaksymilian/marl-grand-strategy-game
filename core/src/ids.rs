/// Identifies a building within one settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildingId(pub u32);

/// Identifies a construction or upgrade order within one settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectId(pub u32);

/// Identifies one persistent settlement within a simulation world.
/// ```compile_fail
/// use simulation_core::ids::{BuildingId, SettlementId};
/// let settlement_id: SettlementId = BuildingId(1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SettlementId(pub u32);

impl std::fmt::Display for SettlementId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
