//! Builds a nation's observation from the authoritative world.
//!
//! Information boundary: only the observed nation, its own settlements (a short
//! summary, not their full observations) and the IDs of other nations are read.
//! Another nation's treasury, stock and policy never enter the result.
use super::nation_observation::{
    ArmyObservation, DecreesObservation, DiplomacyObservation, MilitaryObservation,
    NationObservation, OwnedSettlementObservation, RelationObservation,
};
use crate::domain::ids::NationId;
use crate::domain::nation::diplomacy::Proposal;
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
        decrees: observe_decrees(world, nation_id)?,
        diplomacy: observe_diplomacy(world, nation_id),
        military: observe_military(world, nation_id)?,
        last_turn: last_turn.cloned(),
    })
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

/// Only proposals this nation sent or received; others' negotiations stay hidden.
fn observe_diplomacy(world: &World, nation_id: NationId) -> DiplomacyObservation {
    let others: Vec<_> = world
        .nations()
        .iter()
        .map(|nation| nation.id())
        .filter(|&id| id != nation_id)
        .collect();
    let diplomacy = world.diplomacy();
    let own = |filter: fn(&Proposal, NationId) -> bool| {
        diplomacy
            .proposals()
            .iter()
            .filter(|proposal| filter(proposal, nation_id))
            .copied()
            .collect()
    };
    DiplomacyObservation {
        relations: others
            .iter()
            .map(|&other| {
                let status = RelationObservation {
                    relation: diplomacy.relation(nation_id, other),
                    since_turn: diplomacy.relation_since(nation_id, other),
                };
                (other, status)
            })
            .collect(),
        warning_period: others
            .iter()
            .copied()
            .filter(|&other| diplomacy.in_warning_period(nation_id, other))
            .collect(),
        incoming_proposals: own(|proposal, nation| proposal.to == nation),
        outgoing_proposals: own(|proposal, nation| proposal.from == nation),
        reputation: world
            .nations()
            .iter()
            .map(|nation| (nation.id(), diplomacy.reputation(nation.id())))
            .collect(),
    }
}

/// Only own armies and recruitments; other nations' troops are not shown here.
fn observe_military(world: &World, nation_id: NationId) -> Result<MilitaryObservation, WorldError> {
    let (food_upkeep, gold_upkeep) = world.army_upkeep_demand(nation_id);
    Ok(MilitaryObservation {
        armies: world
            .armies()
            .of_nation(nation_id)
            .map(|army| ArmyObservation {
                army_id: army.id(),
                province_id: army.province(),
                movement: world.army_movement(army),
                soldiers: army.soldiers(),
                units: army.units().to_vec(),
            })
            .collect(),
        recruitments: world
            .armies()
            .recruitments()
            .iter()
            .filter(|recruitment| recruitment.nation == nation_id)
            .cloned()
            .collect(),
        conscription: world
            .nation_settlements(nation_id)?
            .into_iter()
            .filter_map(|id| Some((id, world.conscription(id)?)))
            .collect(),
        food_upkeep,
        gold_upkeep,
    })
}
