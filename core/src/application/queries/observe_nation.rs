//! Builds a nation's observation from the authoritative world.
//!
//! Information boundary: only the observed nation, its own settlements (a short
//! summary, not their full observations) and the IDs of other nations are read.
//! Another nation's treasury, stock and policy never enter the result.
use super::nation_observation::{
    DecreesObservation, DiplomacyObservation, NationObservation, OwnedSettlementObservation,
};
use crate::domain::ids::NationId;
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
        diplomacy: diplomacy(world, nation_id),
        decrees: observe_decrees(world, nation_id)?,
        last_turn: last_turn.cloned(),
    })
}

/// Only proposals this nation sent or received; others' negotiations stay hidden.
fn diplomacy(world: &World, nation_id: NationId) -> DiplomacyObservation {
    let others: Vec<_> = world
        .nations()
        .iter()
        .map(|nation| nation.id())
        .filter(|&id| id != nation_id)
        .collect();
    let diplomacy = world.diplomacy();
    DiplomacyObservation {
        relations: others
            .iter()
            .map(|&other| (other, diplomacy.relation(nation_id, other)))
            .collect(),
        warning_period: others
            .iter()
            .copied()
            .filter(|&other| diplomacy.in_warning_period(nation_id, other))
            .collect(),
        incoming_proposals: diplomacy
            .proposals()
            .iter()
            .filter(|proposal| proposal.to == nation_id)
            .copied()
            .collect(),
        outgoing_proposals: diplomacy
            .proposals()
            .iter()
            .filter(|proposal| proposal.from == nation_id)
            .copied()
            .collect(),
        reputation: world
            .nations()
            .iter()
            .map(|nation| (nation.id(), diplomacy.reputation(nation.id())))
            .collect(),
    }
}

fn observe_decrees(world: &World, nation_id: NationId) -> Result<DecreesObservation, WorldError> {
    let nation = world
        .nation(nation_id)
        .ok_or(WorldError::MissingNation(nation_id))?;
    let catalog = world.decree_catalog();
    Ok(DecreesObservation {
        slots: world.decree_slots(nation_id)?,
        used_slots: nation.decrees().used_slots(),
        minimum_active_turns: catalog.rules().minimum_active_turns,
        cooldown_turns: catalog.rules().cooldown_turns,
        states: nation.decrees().states().clone(),
        upkeep: catalog
            .definitions()
            .iter()
            .map(|definition| {
                let decree = definition.id().clone();
                let upkeep = world.decree_upkeep(nation_id, &decree)?;
                Ok((decree, upkeep))
            })
            .collect::<Result<_, WorldError>>()?,
        effects: world.nation_decree_effects(nation_id)?,
    })
}
