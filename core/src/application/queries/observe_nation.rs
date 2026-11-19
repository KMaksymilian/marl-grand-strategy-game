//! Builds a nation's observation from the authoritative world.
//!
//! Information boundary: only the observed nation, its own settlements (a short
//! summary, not their full observations) and the IDs of other nations are read.
//! Another nation's treasury, stock and policy never enter the result.
use super::nation_observation::{
    DecreesObservation, NationObservation, OwnedSettlementObservation,
};
use crate::domain::ids::NationId;
use crate::domain::nation::decree::Decree;
use crate::domain::nation::report::NationTurnReport;
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
                conquest_turns: ownership.conquest_turns(),
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
        stock: nation.stock_amounts(),
        stock_total: nation.stock().total_amount(),
        stock_capacity: nation.stock().capacity(),
        settlements,
        other_nations: world
            .nations()
            .iter()
            .map(|other| other.id())
            .filter(|&id| id != nation_id)
            .collect(),
        decrees: DecreesObservation {
            slots: world.decree_slots(nation_id)?,
            used_slots: nation.decrees().used_slots(),
            states: nation.decrees().states().clone(),
            costs: Decree::ALL
                .into_iter()
                .map(|decree| Ok((decree, world.decree_cost(nation_id, decree)?)))
                .collect::<Result<_, WorldError>>()?,
            relocation_cost: world.capital_relocation_cost(nation_id)?,
            pending_relocation: nation.decrees().relocation(),
            effects: nation.decrees().effects(),
        },
        last_turn: last_turn.cloned(),
    })
}
