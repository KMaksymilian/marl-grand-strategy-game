//! Decree lifecycle, effects lookup and capital relocation (gameplay
//! specification §7).
//!
//! World owns these operations because they need both aggregates: costs scale
//! with the number of owned settlements, slots follow the settlements'
//! happiness, and a relocation checks the target's ownership and siege state.
//! Every method checks first and changes nothing on an error; the application
//! validator reports the precise reasons to agents.
use super::{World, WorldError};
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::decree::{
    CapitalRelocation, Decree, DecreeBook, DecreeEffects, DecreeState, RelocationFailure,
    RelocationOutcome,
};
use std::collections::BTreeMap;

impl World {
    /// Combined effects of the active decrees of the settlement's owner; no
    /// effects for an unknown settlement.
    // ECONOMY (production, construction, trade, happiness, population): the
    // systems that a decree changes call this for a settlement and apply the
    // fields they own (see `DecreeEffects`).
    pub fn decree_effects(&self, settlement_id: SettlementId) -> DecreeEffects {
        self.settlement_owner(settlement_id)
            .and_then(|nation_id| self.nation(nation_id))
            .map_or_else(DecreeEffects::default, |nation| nation.decrees().effects())
    }

    /// Decree slots of a nation, from the average happiness of its settlements.
    pub fn decree_slots(&self, nation_id: NationId) -> Result<usize, WorldError> {
        self.nation(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))?;
        // ECONOMY (happiness, §4.2 and §7): settlements have no happiness yet,
        // so there is no average and every nation has the base slots. With
        // happiness, pass the average over `nation_settlements(nation_id)` here.
        let average_happiness = None;
        Ok(DecreeBook::slots(average_happiness))
    }

    /// One-time cost of enacting a decree for this nation now.
    pub fn decree_cost(&self, nation_id: NationId, decree: Decree) -> Result<u64, WorldError> {
        Ok(decree.cost(self.nation_settlements(nation_id)?.len()))
    }

    /// Cost of a capital relocation for this nation now.
    pub fn capital_relocation_cost(&self, nation_id: NationId) -> Result<u64, WorldError> {
        Ok(CapitalRelocation::cost(
            self.nation_settlements(nation_id)?.len(),
        ))
    }

    /// Pays the one-time cost and starts a decree: pending this turn, active
    /// from the next. Needs a free slot, a decree that is neither pending,
    /// active nor cooling down, and the gold. Returns the cost paid.
    pub(crate) fn enact_decree(
        &mut self,
        nation_id: NationId,
        decree: Decree,
    ) -> Result<u32, WorldError> {
        let slots = self.decree_slots(nation_id)?;
        let cost = u32::try_from(self.decree_cost(nation_id, decree)?)
            .map_err(|_| WorldError::InsufficientNationalFunds(nation_id))?;
        let nation = self.nation_mut(nation_id)?;
        let book = nation.decrees();
        if book.state(decree).is_some() || book.used_slots() >= slots {
            return Err(WorldError::DecreeUnavailable { nation_id, decree });
        }
        if !nation.withdraw_gold(cost) {
            return Err(WorldError::InsufficientNationalFunds(nation_id));
        }
        nation.decrees_mut().enact(decree);
        Ok(cost)
    }

    /// Ends a decree that has been active for the minimum duration; its
    /// cooldown starts and its slot is free immediately. Nothing is refunded.
    pub(crate) fn revoke_decree(
        &mut self,
        nation_id: NationId,
        decree: Decree,
    ) -> Result<(), WorldError> {
        let book = self.nation_mut(nation_id)?.decrees_mut();
        match book.state(decree) {
            Some(DecreeState::Active { turns_active })
                if turns_active >= Decree::MINIMUM_ACTIVE_TURNS =>
            {
                book.revoke(decree);
                Ok(())
            }
            _ => Err(WorldError::DecreeUnavailable { nation_id, decree }),
        }
    }

    /// Pays a capital relocation now; the capital moves at the start of the
    /// next turn if the target still qualifies, otherwise the cost is refunded.
    /// The target must be an own settlement that is not the capital and not
    /// under attack, and no other relocation may be pending. Returns the cost.
    pub(crate) fn order_capital_relocation(
        &mut self,
        nation_id: NationId,
        target: SettlementId,
    ) -> Result<u32, WorldError> {
        let cost = u32::try_from(self.capital_relocation_cost(nation_id)?)
            .map_err(|_| WorldError::InsufficientNationalFunds(nation_id))?;
        let ownership = *self
            .nations
            .ownership(target)
            .ok_or(WorldError::MissingSettlement(target))?;
        let nation = self.nation_mut(nation_id)?;
        if ownership.nation() != nation_id
            || ownership.is_under_attack()
            || nation.capital() == target
            || nation.decrees().relocation().is_some()
        {
            return Err(WorldError::CapitalRelocationUnavailable {
                nation_id,
                settlement_id: target,
            });
        }
        if !nation.withdraw_gold(cost) {
            return Err(WorldError::InsufficientNationalFunds(nation_id));
        }
        nation
            .decrees_mut()
            .set_relocation(CapitalRelocation { target, paid: cost });
        Ok(cost)
    }

    /// Start of a turn: every relocation ordered last turn either moves the
    /// capital (a chosen move, so no capital-loss penalty) or expires with a
    /// full refund.
    pub(crate) fn resolve_capital_relocations(&mut self) -> BTreeMap<NationId, RelocationOutcome> {
        let ids: Vec<_> = self.nations.iter().map(|nation| nation.id()).collect();
        let mut outcomes = BTreeMap::new();
        for nation_id in ids {
            let relocation = self
                .nation_mut(nation_id)
                .expect("listed above")
                .decrees_mut()
                .take_relocation();
            let Some(relocation) = relocation else {
                continue;
            };
            let target = relocation.target;
            let failure = match self.nations.ownership(target) {
                Some(ownership) if ownership.nation() != nation_id => {
                    Some(RelocationFailure::NotOwned)
                }
                None => Some(RelocationFailure::NotOwned),
                Some(ownership) if ownership.is_under_attack() => {
                    Some(RelocationFailure::UnderAttack)
                }
                Some(_) => None,
            };
            let nation = self.nation_mut(nation_id).expect("listed above");
            let outcome = match failure {
                None => {
                    let from = nation.capital();
                    nation.relocate_capital(target);
                    RelocationOutcome::Completed { from, to: target }
                }
                Some(reason) => RelocationOutcome::Refunded {
                    target,
                    reason,
                    refund: nation.deposit_gold(relocation.paid),
                },
            };
            outcomes.insert(nation_id, outcome);
        }
        outcomes
    }
}
