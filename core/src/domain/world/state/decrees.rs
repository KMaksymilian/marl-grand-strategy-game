//! Decree lifecycle and effects lookup (gameplay specification §7).
//!
//! World owns these operations because they need the decree catalogue and both
//! aggregates: upkeep scales with the number of owned settlements and slots
//! follow the settlements' happiness. Every method checks first and changes
//! nothing on an error.
use super::{NationOperationError, World, WorldError};
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::decree::{DecreeEffects, DecreeId, DecreeState, DecreeUpkeepReport};
use crate::domain::nation::decree_catalog::DecreeCatalog;
use std::collections::BTreeMap;
use std::sync::Arc;

impl World {
    /// Decrees nations may enact and their common rules.
    pub fn decree_catalog(&self) -> &DecreeCatalog {
        &self.decree_catalog
    }

    /// Replaces the bundled decree catalogue, e.g. with `decrees.toml` loaded
    /// at startup. Refused once any nation has a decree with a state, because
    /// its IDs and upkeep come from the catalogue in use.
    pub fn set_decree_catalog(&mut self, catalog: Arc<DecreeCatalog>) -> Result<(), WorldError> {
        if self
            .nations
            .iter()
            .any(|nation| !nation.decrees().states().is_empty())
        {
            return Err(WorldError::DecreeCatalogInUse);
        }
        self.decree_catalog = catalog;
        Ok(())
    }

    /// Combined effects of the active decrees of the settlement's owner; no
    /// effects for an unknown settlement.
    // ECONOMY (production, construction, trade, happiness, population): the
    // systems that a decree changes call this for a settlement and apply the
    // fields they own (see `DecreeEffects`).
    pub fn decree_effects(&self, settlement_id: SettlementId) -> DecreeEffects {
        self.settlement_owner(settlement_id)
            .and_then(|nation_id| self.nation_decree_effects(nation_id).ok())
            .unwrap_or_default()
    }

    /// Combined effects of a nation's active decrees.
    pub fn nation_decree_effects(&self, nation_id: NationId) -> Result<DecreeEffects, WorldError> {
        let nation = self
            .nation(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))?;
        Ok(self.decree_catalog.effects_of(nation.decrees().active()))
    }

    /// Decree slots of a nation, from the average happiness of its settlements.
    pub fn decree_slots(&self, nation_id: NationId) -> Result<usize, WorldError> {
        let mut sum = 0.0;
        let mut count = 0;
        for id in self.nation_settlements(nation_id)? {
            if let Some(happiness) = self.settlement_happiness(id)? {
                sum += happiness.score();
                count += 1;
            }
        }
        let average_happiness = (count > 0).then(|| (sum / f64::from(count)) as f32);
        Ok(self.decree_catalog.rules().slots(average_happiness))
    }

    /// Gold per active turn of one decree for this nation now.
    pub fn decree_upkeep(&self, nation_id: NationId, decree: &DecreeId) -> Result<u64, WorldError> {
        let settlements = self.nation_settlements(nation_id)?.len();
        self.decree_catalog
            .upkeep(decree, settlements)
            .ok_or_else(|| WorldError::UnknownDecree(decree.clone()))
    }

    /// Starts a decree: pending this turn, active (and paid) from the next.
    /// Needs a free slot and a decree that is neither pending, active nor
    /// cooling down. Costs nothing now; returns the upkeep per turn.
    pub(crate) fn enact_decree(
        &mut self,
        nation_id: NationId,
        decree: DecreeId,
    ) -> Result<u64, NationOperationError> {
        if self.decree_catalog.get(&decree).is_none() {
            return Err(NationOperationError::UnknownDecree(decree));
        }
        let nation = self
            .nation(nation_id)
            .ok_or(NationOperationError::MissingNation(nation_id))?;
        match nation.decrees().state(&decree) {
            Some(DecreeState::Pending | DecreeState::Active { .. }) => {
                return Err(NationOperationError::DecreeAlreadyEnacted(decree));
            }
            Some(DecreeState::Cooldown { turns_left }) => {
                return Err(NationOperationError::DecreeOnCooldown { decree, turns_left });
            }
            None => {}
        }
        let slots = self.decree_slots(nation_id).expect("nation exists");
        if nation.decrees().used_slots() >= slots {
            return Err(NationOperationError::NoFreeDecreeSlot(slots));
        }
        let upkeep = self
            .decree_upkeep(nation_id, &decree)
            .expect("nation and decree exist");
        self.nation_mut(nation_id)
            .expect("nation exists")
            .decrees_mut()
            .enact(decree);
        Ok(upkeep)
    }

    /// Ends a decree that has been active for the minimum duration; its
    /// cooldown starts and its slot is free immediately. Nothing is refunded.
    /// Returns the cooldown in turns.
    pub(crate) fn revoke_decree(
        &mut self,
        nation_id: NationId,
        decree: &DecreeId,
    ) -> Result<u32, NationOperationError> {
        if self.decree_catalog.get(decree).is_none() {
            return Err(NationOperationError::UnknownDecree(decree.clone()));
        }
        let rules = *self.decree_catalog.rules();
        let nation = self
            .nations
            .get_mut(nation_id)
            .ok_or(NationOperationError::MissingNation(nation_id))?;
        match nation.decrees().state(decree) {
            Some(DecreeState::Active { turns_active })
                if turns_active < rules.minimum_active_turns =>
            {
                Err(NationOperationError::DecreeMinimumDuration {
                    decree: decree.clone(),
                    turns_left: rules.minimum_active_turns - turns_active,
                })
            }
            Some(DecreeState::Active { .. }) => {
                nation.decrees_mut().revoke(decree, rules.cooldown_turns);
                Ok(rules.cooldown_turns)
            }
            _ => Err(NationOperationError::DecreeNotActive(decree.clone())),
        }
    }

    /// Every turn, right after nation decisions: each active decree costs its
    /// upkeep, in ascending ID. A decree the treasury cannot pay is revoked,
    /// exactly as if the nation had revoked it: its cooldown starts and it has
    /// no effect this turn.
    pub(crate) fn pay_decree_upkeep(&mut self) -> BTreeMap<NationId, DecreeUpkeepReport> {
        let catalog = Arc::clone(&self.decree_catalog);
        let nations: Vec<_> = self
            .nations
            .iter()
            .map(|nation| (nation.id(), self.nations.settlements_of(nation.id()).len()))
            .collect();
        let mut reports = BTreeMap::new();
        for (nation_id, settlements) in nations {
            let nation = self.nation_mut(nation_id).expect("listed above");
            let mut report = DecreeUpkeepReport::default();
            let active: Vec<_> = nation.decrees().active().cloned().collect();
            for decree in active {
                // A decree with a state is always in the catalogue: the
                // catalogue cannot be replaced while any decree has a state.
                let upkeep = catalog.upkeep(&decree, settlements).unwrap_or(0);
                let paid = u32::try_from(upkeep)
                    .ok()
                    .is_some_and(|upkeep| nation.withdraw_gold(upkeep));
                if paid {
                    report.gold_paid += upkeep;
                    report.paid.push(decree);
                } else {
                    nation
                        .decrees_mut()
                        .revoke(&decree, catalog.rules().cooldown_turns);
                    report.revoked_unpaid.push(decree);
                }
            }
            reports.insert(nation_id, report);
        }
        reports
    }
}
