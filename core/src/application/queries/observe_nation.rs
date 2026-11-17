//! Builds a nation's observation from the authoritative world.
//!
//! Information boundary: only the observed nation, its own settlements (a short
//! summary, not their full observations) and the IDs of other nations are read.
//! Another nation's treasury, stock and policy never enter the result; a test
//! compares observations of worlds that differ only in those values.
use super::nation_observation::{NationObservation, OwnedSettlementObservation};
use crate::domain::ids::NationId;
use crate::domain::nation::taxation::NationTurnReport;
use crate::domain::world::{World, WorldError};

pub(crate) fn observe_nation(
    world: &World,
    nation_id: NationId,
    turn: u64,
    last_turn: Option<&NationTurnReport>,
) -> Result<NationObservation, WorldError> {
    let nation = world
        .nation(nation_id)
        .ok_or(WorldError::MissingNation(nation_id))?;
    let settlements = world
        .nation_settlements(nation_id)?
        .into_iter()
        .filter_map(|settlement_id| {
            let settlement = world.settlement(settlement_id)?;
            let inventory = settlement.inventory();
            let ownership = world.nations().ownership(settlement_id)?;
            Some(OwnedSettlementObservation {
                settlement_id,
                province_id: world.settlement_province(settlement_id)?,
                population: settlement.population(),
                gold: settlement.gold(),
                conquest_penalty: ownership.conquest_penalty(),
                under_attack: ownership.is_under_attack(),
                free_capacity: u64::from(inventory.capacity())
                    .saturating_sub(inventory.total_amount()),
            })
        })
        .collect();
    Ok(NationObservation {
        turn,
        nation_id,
        capital: nation.capital(),
        capital_loss_turns: nation.capital_loss_turns(),
        tax_rate: nation.tax_rate(),
        treasury: nation.treasury(),
        stock: nation.stock().resources().collect(),
        stock_capacity_per_resource: world.nation_stock_capacity(nation_id)?,
        settlements,
        other_nations: world
            .nations()
            .iter()
            .map(|other| other.id())
            .filter(|&id| id != nation_id)
            .collect(),
        last_turn: last_turn.cloned(),
    })
}
