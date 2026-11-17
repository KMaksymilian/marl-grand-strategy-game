//! Decree lifecycle, upkeep and capital relocation (gameplay specification §7).
//!
//! World owns these operations because they need both aggregates: upkeep scales
//! with the number of owned settlements, and a relocation checks the target's
//! ownership and siege state. Every method checks first and changes nothing on
//! an error. The application validator reports the precise reasons to agents.
use super::{World, WorldError};
use crate::domain::buildings::definition::BuildingDefinition;
use crate::domain::economy::cost::ProjectCost;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::decree::{
    CapitalRelocation, Decree, DecreeBook, DecreeEffects, DecreeState, DecreeTurnReport,
    RelocationFailure, RelocationOutcome,
};
use std::collections::BTreeMap;

impl World {
    /// Combined effects of the active decrees of the settlement's owner; no
    /// effects for an unknown settlement. Owning systems read this.
    pub fn decree_effects(&self, settlement_id: SettlementId) -> DecreeEffects {
        self.settlement_owner(settlement_id)
            .and_then(|nation_id| self.nation(nation_id))
            .map_or_else(DecreeEffects::default, |nation| nation.decrees().effects())
    }

    /// The material cost of building `definition` for this settlement: the
    /// catalog cost changed by the owner's decrees (e.g. −20% under a great
    /// construction), each material rounded up. Turns are unchanged.
    pub fn construction_cost(
        &self,
        settlement_id: SettlementId,
        definition: &BuildingDefinition,
    ) -> ProjectCost {
        let change = self.decree_effects(settlement_id).construction_cost;
        let mut cost = definition.construction_cost().clone();
        if change != 0.0 {
            let factor = (1.0 + change).max(0.0);
            for amount in cost.needed_resources.values_mut() {
                // Relative tolerance: 200 × 0.8 must stay 160, not become 161.
                let scaled = (f64::from(*amount) * factor * (1.0 - 1e-9)).ceil();
                *amount = scaled as u32;
            }
        }
        cost
    }

    /// Decree slots of a nation. Settlement happiness is not modelled yet, so
    /// every nation has the base slots; with happiness this becomes the slots
    /// for the average happiness of its settlements.
    pub fn decree_slots(&self, nation_id: NationId) -> Result<usize, WorldError> {
        self.nation(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))?;
        Ok(DecreeBook::slots(None))
    }

    /// Gold per turn of one decree for this nation now.
    pub fn decree_upkeep(&self, nation_id: NationId, decree: Decree) -> Result<u64, WorldError> {
        Ok(decree.upkeep(self.nation_settlements(nation_id)?.len()))
    }

    /// Gold a capital relocation costs this nation now.
    pub fn capital_relocation_cost(&self, nation_id: NationId) -> Result<u64, WorldError> {
        Ok(CapitalRelocation::cost(
            self.nation_settlements(nation_id)?.len(),
        ))
    }

    /// Starts a decree: pending this turn, active from the next. Needs a free
    /// slot and a decree that is neither pending, active nor cooling down.
    pub(crate) fn enact_decree(
        &mut self,
        nation_id: NationId,
        decree: Decree,
    ) -> Result<(), WorldError> {
        let slots = self.decree_slots(nation_id)?;
        let book = self.nation_mut(nation_id)?.decrees_mut();
        if book.state(decree).is_some() || book.used_slots() >= slots {
            return Err(WorldError::DecreeUnavailable { nation_id, decree });
        }
        book.enact(decree);
        Ok(())
    }

    /// Ends a decree that has been active for the minimum duration; its
    /// cooldown starts and its slot is free immediately.
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
                book.end(decree);
                Ok(())
            }
            _ => Err(WorldError::DecreeUnavailable { nation_id, decree }),
        }
    }

    /// Pays a capital relocation now; the capital moves at the start of the
    /// next turn if the target still qualifies, otherwise the cost is refunded.
    /// The target must be an own settlement that is not the capital and not
    /// under attack, and no other relocation may be pending.
    pub(crate) fn order_capital_relocation(
        &mut self,
        nation_id: NationId,
        target: SettlementId,
    ) -> Result<u32, WorldError> {
        let cost = self.capital_relocation_cost(nation_id)?;
        let unavailable = WorldError::CapitalRelocationUnavailable {
            nation_id,
            settlement_id: target,
        };
        let ownership = self
            .nations
            .ownership(target)
            .ok_or(WorldError::MissingSettlement(target))?;
        let nation = self.nation(nation_id).expect("checked by the cost");
        if ownership.nation() != nation_id
            || ownership.is_under_attack()
            || nation.capital() == target
            || nation.decrees().relocation().is_some()
        {
            return Err(unavailable);
        }
        let cost =
            u32::try_from(cost).map_err(|_| WorldError::InsufficientNationalFunds(nation_id))?;
        let nation = self.nation_mut(nation_id)?;
        if !nation.withdraw_gold(cost) {
            return Err(WorldError::InsufficientNationalFunds(nation_id));
        }
        nation
            .decrees_mut()
            .set_relocation(CapitalRelocation { target, paid: cost });
        Ok(cost)
    }

    /// Start of a turn: every relocation ordered last turn either moves the
    /// capital (no capital-loss penalty) or expires with a full refund.
    pub(crate) fn resolve_capital_relocations(&mut self) -> BTreeMap<NationId, RelocationOutcome> {
        let ids: Vec<_> = self.nations.iter().map(|nation| nation.id()).collect();
        let mut outcomes = BTreeMap::new();
        for nation_id in ids {
            let nation = self.nations.get_mut(nation_id).expect("listed above");
            let Some(relocation) = nation.decrees_mut().take_relocation() else {
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
            let nation = self.nations.get_mut(nation_id).expect("listed above");
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

    /// After nation decisions: every active decree costs its upkeep, in
    /// `Decree` order. A decree the treasury cannot pay lapses: it ends and its
    /// cooldown starts, so it has no effect this turn.
    pub(crate) fn pay_decree_upkeep(&mut self) -> BTreeMap<NationId, DecreeTurnReport> {
        let nations: Vec<_> = self
            .nations
            .iter()
            .map(|nation| (nation.id(), self.nations.settlements_of(nation.id()).len()))
            .collect();
        let mut reports = BTreeMap::new();
        for (nation_id, settlements) in nations {
            let nation = self.nations.get_mut(nation_id).expect("listed above");
            let mut report = DecreeTurnReport::default();
            let active: Vec<_> = nation.decrees().active().collect();
            for decree in active {
                let upkeep = decree.upkeep(settlements);
                let paid = u32::try_from(upkeep)
                    .ok()
                    .is_some_and(|upkeep| nation.withdraw_gold(upkeep));
                if paid {
                    report.upkeep_paid += upkeep;
                    report.active.push(decree);
                } else {
                    nation.decrees_mut().end(decree);
                    report.lapsed.push(decree);
                }
            }
            reports.insert(nation_id, report);
        }
        reports
    }
}
