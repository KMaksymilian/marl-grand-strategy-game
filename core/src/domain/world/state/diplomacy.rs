//! Diplomacy in the world (gameplay specification §8): relation lookups, the
//! trade block of wars and embargoes, and diplomatic actions with the checks
//! that need the rest of the game (whether the nations exist).
use super::World;
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::diplomacy::{
    Diplomacy, DiplomacyError, DiplomaticAction, DiplomaticEvent, DiplomaticOutcome, Relation,
};

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

    /// Whether two settlements may trade: always within one nation, otherwise
    /// unless their nations are at war or under an embargo (§8.1).
    pub fn trade_allowed(&self, sender: SettlementId, receiver: SettlementId) -> bool {
        match (
            self.settlement_owner(sender),
            self.settlement_owner(receiver),
        ) {
            (Some(a), Some(b)) if a != b => self.diplomacy.relation(a, b).allows_trade(),
            _ => true,
        }
    }

    /// Whether the nation may take the action now.
    pub(crate) fn check_diplomatic_action(
        &self,
        nation_id: NationId,
        action: DiplomaticAction,
    ) -> Result<(), DiplomacyError> {
        let other = action.other();
        if self.nation(other).is_none() {
            return Err(DiplomacyError::UnknownNation { nation_id: other });
        }
        self.diplomacy.check(nation_id, action)
    }

    /// Checks and applies the action. A peace keeps every settlement with its
    /// current owner (§8.5).
    // ARMY (§8.5): armies standing on the other side's territory return to the
    // nearest own province after a peace; armies do not exist yet.
    pub(crate) fn apply_diplomatic_action(
        &mut self,
        nation_id: NationId,
        action: DiplomaticAction,
    ) -> Result<DiplomaticOutcome, DiplomacyError> {
        self.check_diplomatic_action(nation_id, action)?;
        let nations: Vec<_> = self.nations.iter().map(|nation| nation.id()).collect();
        self.diplomacy.apply(nation_id, action, &nations)
    }

    /// Diplomatic events since the last call, for the nations' turn reports.
    pub(crate) fn take_diplomatic_events(&mut self) -> Vec<(NationId, DiplomaticEvent)> {
        self.diplomacy.take_events()
    }
}
