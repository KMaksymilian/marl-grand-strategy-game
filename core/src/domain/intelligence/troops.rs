//! Troop presence (ADR 0006, MGS-45): who saw which armies in which province.
//!
//! An observation records every army standing in one province: its nation
//! and living soldiers, without the composition of units. It is a fact of
//! the moment; recipients hide their own and their allies' armies when they
//! read it. An observation with no troops is an observed absence; a province
//! without any observation is unobserved.
use crate::domain::ids::{ArmyId, NationId, SettlementId};
use serde::{Deserialize, Serialize};

/// One nation's armies in an observed province, together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TroopSighting {
    pub nation_id: NationId,
    pub soldiers: u32,
}

/// Who made a troop observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "observer", rename_all = "snake_case", deny_unknown_fields)]
pub enum TroopSource {
    /// An army sees its province and the neighbouring ones; its nation knows
    /// at once.
    Army { army_id: ArmyId },
    /// A settlement sees its province and the neighbouring ones; it and its
    /// nation know at once.
    Settlement { settlement_id: SettlementId },
    /// A caravan sees the provinces it passes; the sending settlement knows
    /// when it returns.
    Caravan { sender_id: SettlementId },
}
