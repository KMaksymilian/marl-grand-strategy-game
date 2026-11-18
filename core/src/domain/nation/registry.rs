//! The ordered store of nations and the single ownership record
//! `settlement -> nation`. Every registered settlement belongs to exactly one
//! nation; there are no independent settlements.
use super::Nation;
use crate::domain::ids::{NationId, SettlementId};
use std::collections::BTreeMap;

/// Read-only public access; changes go through World.
/// ```compile_fail
/// use simulation_core::nation::registry::NationRegistry;
/// use simulation_core::ids::NationId;
/// let mut registry = NationRegistry::default();
/// registry.get_mut(NationId(0));
/// ```
#[derive(Debug)]
pub struct NationRegistry {
    /// BTreeMap so iteration (turn order, observations) is by ascending ID.
    nations: BTreeMap<NationId, Nation>,
    /// The only ownership record. Every registered settlement has an entry.
    /// Keyed by settlement, so one settlement can never have two owners.
    owners: BTreeMap<SettlementId, NationId>,
    /// Next ID to hand out; None once u32 is exhausted. IDs of removed nations
    /// are never reused, so an old ID in a report or an agent's memory cannot
    /// point at a different, newer nation.
    next_id: Option<NationId>,
}

impl Default for NationRegistry {
    fn default() -> Self {
        Self {
            nations: BTreeMap::new(),
            owners: BTreeMap::new(),
            next_id: Some(NationId(0)),
        }
    }
}

impl NationRegistry {
    pub fn get(&self, id: NationId) -> Option<&Nation> {
        self.nations.get(&id)
    }

    /// Always ascending by NationId.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Nation> {
        self.nations.values()
    }

    pub fn len(&self) -> usize {
        self.nations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nations.is_empty()
    }

    /// The owning nation; None only for an unregistered settlement ID.
    pub fn owner_of(&self, settlement_id: SettlementId) -> Option<NationId> {
        self.owners.get(&settlement_id).copied()
    }

    /// Settlements owned by the nation, ascending ID. Derived from `owners` on
    /// every call: with tens of settlements a scan is cheaper than keeping a
    /// second index in sync.
    pub fn settlements_of(&self, nation_id: NationId) -> Vec<SettlementId> {
        self.owners
            .iter()
            .filter(|(_, owner)| **owner == nation_id)
            .map(|(&settlement_id, _)| settlement_id)
            .collect()
    }

    /// Per-material stock capacity: a fixed amount for every owned settlement.
    pub fn stock_capacity(&self, nation_id: NationId) -> u32 {
        let settlements = u32::try_from(self.settlements_of(nation_id).len()).unwrap_or(u32::MAX);
        settlements.saturating_mul(Nation::STOCK_CAPACITY_PER_SETTLEMENT)
    }

    /// Whether `found` can still assign an ID.
    pub(crate) fn can_found(&self) -> bool {
        self.next_id.is_some()
    }

    pub(crate) fn get_mut(&mut self, id: NationId) -> Option<&mut Nation> {
        self.nations.get_mut(&id)
    }

    /// Creates a nation owning one newly registered settlement, which becomes
    /// its capital. The caller checks `can_found` and that the settlement has
    /// no owner yet.
    pub(crate) fn found(&mut self, capital: SettlementId) -> Option<NationId> {
        let id = self.next_id?;
        debug_assert!(!self.owners.contains_key(&capital));
        self.nations.insert(id, Nation::new(id, capital));
        self.owners.insert(capital, id);
        self.next_id = id.0.checked_add(1).map(NationId);
        Some(id)
    }

    /// Adds a newly registered settlement to an existing nation.
    pub(crate) fn add_member(&mut self, settlement_id: SettlementId, nation_id: NationId) {
        debug_assert!(self.nations.contains_key(&nation_id));
        debug_assert!(!self.owners.contains_key(&settlement_id));
        self.owners.insert(settlement_id, nation_id);
    }

    /// Records a new owner of an existing settlement and returns the previous one.
    pub(crate) fn set_owner(
        &mut self,
        settlement_id: SettlementId,
        nation_id: NationId,
    ) -> Option<NationId> {
        self.owners.insert(settlement_id, nation_id)
    }

    /// Removes a nation that owns nothing any more. IDs are never reused.
    pub(crate) fn remove(&mut self, nation_id: NationId) -> Option<Nation> {
        debug_assert!(self.settlements_of(nation_id).is_empty());
        self.nations.remove(&nation_id)
    }

    /// Counts down per-turn timers of every nation at the end of a turn.
    pub(crate) fn advance_turn(&mut self) {
        for nation in self.nations.values_mut() {
            nation.advance_turn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn founding_assigns_consecutive_ids_and_the_capital() {
        let mut registry = NationRegistry::default();
        let first = registry.found(SettlementId(4)).unwrap();
        let second = registry.found(SettlementId(2)).unwrap();
        assert_eq!((first, second), (NationId(0), NationId(1)));
        assert_eq!(registry.get(first).unwrap().capital(), SettlementId(4));
        assert_eq!(registry.owner_of(SettlementId(2)), Some(second));
        assert_eq!(registry.owner_of(SettlementId(3)), None);
        assert_eq!(registry.stock_capacity(first), 500);
    }

    #[test]
    fn ownership_is_one_record_per_settlement() {
        let mut registry = NationRegistry::default();
        let first = registry.found(SettlementId(1)).unwrap();
        let second = registry.found(SettlementId(2)).unwrap();
        registry.add_member(SettlementId(3), first);
        assert_eq!(registry.set_owner(SettlementId(3), second), Some(first));
        assert_eq!(registry.settlements_of(first), vec![SettlementId(1)]);
        assert_eq!(
            registry.settlements_of(second),
            vec![SettlementId(2), SettlementId(3)]
        );
        assert_eq!(registry.stock_capacity(second), 1_000);
    }

    #[test]
    fn removed_nation_ids_are_not_reused() {
        let mut registry = NationRegistry::default();
        let first = registry.found(SettlementId(1)).unwrap();
        let second = registry.found(SettlementId(2)).unwrap();
        registry.set_owner(SettlementId(1), second);
        assert!(registry.remove(first).is_some());
        assert!(registry.get(first).is_none());
        assert_eq!(registry.found(SettlementId(3)), Some(NationId(2)));
        assert_eq!(registry.len(), 2);
    }
}
