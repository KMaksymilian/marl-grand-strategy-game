//! Diplomacy in the world (gameplay specification §8): relation lookups, the
//! trade block of wars and embargoes, and diplomatic actions with the checks
//! that need the rest of the game (whether the nations exist).
//!
//! Every action is checked and applied at once: an action of a decision sees
//! what earlier actions of the same turn changed (e.g. a war that drew this
//! nation in through an alliance).
use super::nations::NationOperationError;
use super::{World, WorldError};
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::diplomacy::{
    Diplomacy, DiplomacyError, DiplomacyRules, DiplomaticEvent, DiplomaticOutcome, Relation, Treaty,
};
use std::sync::Arc;

/// Read access.
impl World {
    pub fn diplomacy(&self) -> &Diplomacy {
        &self.diplomacy
    }

    /// The relation of two existing nations; None for the same nation.
    pub fn relation(&self, a: NationId, b: NationId) -> Result<Option<Relation>, DiplomacyError> {
        for nation_id in [a, b] {
            if self.nation(nation_id).is_none() {
                return Err(DiplomacyError::UnknownNation { nation_id });
            }
        }
        Ok((a != b).then(|| self.diplomacy.relation(a, b)))
    }

    /// The other nation when two settlements may not trade: their nations are
    /// at war or under an embargo (§8.1). Settlements of one nation always
    /// may; None also for unknown settlements.
    pub fn trade_blocked_by(
        &self,
        sender: SettlementId,
        receiver: SettlementId,
    ) -> Option<NationId> {
        let a = self.settlement_owner(sender)?;
        let b = self.settlement_owner(receiver)?;
        (a != b && !self.diplomacy.relation(a, b).allows_trade()).then_some(b)
    }

    /// The settlement is under attack (§11.2): it does not trade.
    pub fn is_under_attack(&self, settlement: SettlementId) -> bool {
        self.nations
            .ownership(settlement)
            .is_some_and(|ownership| ownership.is_under_attack())
    }

    /// Whether two settlements may trade: their nations allow it (see
    /// `trade_blocked_by`) and neither is under attack.
    pub fn trade_allowed(&self, sender: SettlementId, receiver: SettlementId) -> bool {
        self.trade_blocked_by(sender, receiver).is_none()
            && !self.is_under_attack(sender)
            && !self.is_under_attack(receiver)
    }

    /// Replaces the bundled diplomacy rules, e.g. with `diplomacy.toml` loaded
    /// at startup. Refused once any pair has left neutrality or any proposal
    /// waits, because truces and windows in progress follow the rules in use.
    pub fn set_diplomacy_rules(&mut self, rules: Arc<DiplomacyRules>) -> Result<(), WorldError> {
        if !self.diplomacy.is_untouched() {
            return Err(WorldError::DiplomacyInProgress);
        }
        self.diplomacy.set_rules(rules);
        Ok(())
    }
}

/// Diplomatic actions of a nation, for the nation decision handler.
impl World {
    /// The global turn being played: one after the last completed turn.
    pub(super) fn current_turn(&self) -> u64 {
        self.completed_turn().saturating_add(1)
    }

    /// Both nations exist; returns every nation in the game, for alliances.
    fn diplomatic_parties(
        &self,
        actor: NationId,
        other: NationId,
    ) -> Result<Vec<NationId>, NationOperationError> {
        if self.nation(actor).is_none() {
            return Err(NationOperationError::MissingNation(actor));
        }
        if self.nation(other).is_none() {
            return Err(NationOperationError::Diplomacy(
                DiplomacyError::UnknownNation { nation_id: other },
            ));
        }
        Ok(self.nations.iter().map(|nation| nation.id()).collect())
    }

    /// Both alliances go to war (§8.3).
    pub(crate) fn declare_war(
        &mut self,
        actor: NationId,
        target: NationId,
    ) -> Result<DiplomaticOutcome, NationOperationError> {
        let nations = self.diplomatic_parties(actor, target)?;
        let turn = self.current_turn();
        // ARMY (§11): armies may now enter the other side's provinces.
        Ok(self.diplomacy.declare_war(actor, target, &nations, turn)?)
    }

    pub(crate) fn declare_embargo(
        &mut self,
        actor: NationId,
        target: NationId,
    ) -> Result<DiplomaticOutcome, NationOperationError> {
        self.diplomatic_parties(actor, target)?;
        let turn = self.current_turn();
        Ok(self.diplomacy.declare_embargo(actor, target, turn)?)
    }

    pub(crate) fn lift_embargo(
        &mut self,
        actor: NationId,
        target: NationId,
    ) -> Result<DiplomaticOutcome, NationOperationError> {
        self.diplomatic_parties(actor, target)?;
        let turn = self.current_turn();
        Ok(self.diplomacy.lift_embargo(actor, target, turn)?)
    }

    /// A trade agreement or an alliance.
    pub(crate) fn break_treaty(
        &mut self,
        actor: NationId,
        target: NationId,
        treaty: Treaty,
    ) -> Result<DiplomaticOutcome, NationOperationError> {
        self.diplomatic_parties(actor, target)?;
        let turn = self.current_turn();
        Ok(self.diplomacy.break_treaty(actor, target, treaty, turn)?)
    }

    pub(crate) fn propose_treaty(
        &mut self,
        actor: NationId,
        target: NationId,
        treaty: Treaty,
    ) -> Result<DiplomaticOutcome, NationOperationError> {
        self.diplomatic_parties(actor, target)?;
        Ok(self.diplomacy.propose(actor, target, treaty)?)
    }

    /// A peace keeps every settlement with its current owner (§8.5).
    // ARMY (§8.5): armies standing on the other side's territory return to the
    // nearest own province after a peace.
    pub(crate) fn respond_to_proposal(
        &mut self,
        actor: NationId,
        proposer: NationId,
        treaty: Treaty,
        accept: bool,
    ) -> Result<DiplomaticOutcome, NationOperationError> {
        let nations = self.diplomatic_parties(actor, proposer)?;
        let turn = self.current_turn();
        Ok(self
            .diplomacy
            .respond(actor, proposer, treaty, accept, &nations, turn)?)
    }

    /// Diplomatic events since the last call, for the nations' turn reports.
    pub(crate) fn take_diplomatic_events(&mut self) -> Vec<(NationId, DiplomaticEvent)> {
        self.diplomacy.take_events()
    }
}

impl From<DiplomacyError> for NationOperationError {
    fn from(error: DiplomacyError) -> Self {
        Self::Diplomacy(error)
    }
}
