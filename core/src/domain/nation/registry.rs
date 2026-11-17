//! The ordered store of nations and the single ownership record
//! `settlement -> nation`. Every registered settlement belongs to exactly one
//! nation; there are no independent settlements. Rules that need settlement
//! data (capital succession, extinction) are coordinated by World.
use super::Nation;
use crate::domain::ids::{NationId, SettlementId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Who owns a settlement and its political state towards that owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ownership {
    nation: NationId,
    /// Remaining conquest penalty (§4.2): 20 right after the capture, 5 less
    /// after every turn. While above zero the settlement pays half the tax rate
    /// (§6). Its effect on happiness belongs to the future happiness model.
    conquest_penalty: u32,
    /// An enemy army stands in the settlement's province (§11.2); such a
    /// settlement pays no tax. Set by the war system.
    under_attack: bool,
}

impl Ownership {
    /// Conquest penalty right after a capture (four turns of half tax).
    pub const CONQUEST_PENALTY: u32 = 20;
    /// How much the conquest penalty shrinks every turn.
    pub const CONQUEST_PENALTY_DECAY: u32 = 5;

    fn new(nation: NationId) -> Self {
        Self {
            nation,
            conquest_penalty: 0,
            under_attack: false,
        }
    }

    pub fn nation(&self) -> NationId {
        self.nation
    }

    pub fn conquest_penalty(&self) -> u32 {
        self.conquest_penalty
    }

    /// Freshly conquered: the conquest penalty still applies.
    pub fn is_freshly_conquered(&self) -> bool {
        self.conquest_penalty > 0
    }

    pub fn is_under_attack(&self) -> bool {
        self.under_attack
    }
}

/// Read-only public access; changes go through World.
/// ```compile_fail
/// use simulation_core::nation::registry::NationRegistry;
/// use simulation_core::ids::NationId;
/// let mut registry = NationRegistry::default();
/// registry.get_mut(NationId(0));
/// ```
#[derive(Debug)]
pub struct NationRegistry {
    /// BTreeMap so iteration (turn order, observations) is always by ascending ID.
    nations: BTreeMap<NationId, Nation>,
    /// The only ownership record. Every registered settlement has an entry.
    /// Keyed by settlement, so one settlement can never have two owners.
    owners: BTreeMap<SettlementId, Ownership>,
    /// Next ID to hand out; None once u32 is exhausted. IDs of removed nations
    /// are never reused, so an old ID in a report or a Python agent's memory
    /// can never point at a different, newer nation.
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
        self.owners.get(&settlement_id).map(Ownership::nation)
    }

    /// The full ownership record; None only for an unregistered settlement ID.
    pub fn ownership(&self, settlement_id: SettlementId) -> Option<&Ownership> {
        self.owners.get(&settlement_id)
    }

    /// Settlements owned by the nation, ascending ID.
    /// Derived from `owners` on every call (a linear scan); with tens of
    /// settlements this is cheaper than keeping a second, synchronized index.
    pub fn settlements_of(&self, nation_id: NationId) -> Vec<SettlementId> {
        self.owners
            .iter()
            .filter(|(_, ownership)| ownership.nation == nation_id)
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
        self.owners.insert(capital, Ownership::new(id));
        self.next_id = id.0.checked_add(1).map(NationId);
        Some(id)
    }

    /// Adds a newly registered settlement to an existing nation, without any
    /// conquest penalty (scenario setup).
    pub(crate) fn add_member(&mut self, settlement_id: SettlementId, nation_id: NationId) {
        debug_assert!(self.nations.contains_key(&nation_id));
        debug_assert!(!self.owners.contains_key(&settlement_id));
        self.owners.insert(settlement_id, Ownership::new(nation_id));
    }

    /// Gives the settlement to a new owner after a conquest and returns the
    /// previous owner. Starts the conquest penalty; the siege is over.
    pub(crate) fn capture(
        &mut self,
        settlement_id: SettlementId,
        nation_id: NationId,
    ) -> Option<NationId> {
        let previous = self.owner_of(settlement_id);
        self.owners.insert(
            settlement_id,
            Ownership {
                nation: nation_id,
                conquest_penalty: Ownership::CONQUEST_PENALTY,
                under_attack: false,
            },
        );
        previous
    }

    /// Returns false for an unregistered settlement.
    pub(crate) fn set_under_attack(&mut self, settlement_id: SettlementId, attacked: bool) -> bool {
        match self.owners.get_mut(&settlement_id) {
            Some(ownership) => {
                ownership.under_attack = attacked;
                true
            }
            None => false,
        }
    }

    /// Removes a nation that owns nothing any more. IDs are never reused.
    pub(crate) fn remove(&mut self, nation_id: NationId) -> Option<Nation> {
        debug_assert!(self.settlements_of(nation_id).is_empty());
        self.nations.remove(&nation_id)
    }

    /// Counts down per-turn timers of nations and ownerships at the end of a turn.
    pub(crate) fn advance_turn(&mut self) {
        for nation in self.nations.values_mut() {
            nation.advance_turn();
        }
        for ownership in self.owners.values_mut() {
            ownership.conquest_penalty = ownership
                .conquest_penalty
                .saturating_sub(Ownership::CONQUEST_PENALTY_DECAY);
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
        assert_eq!(registry.capture(SettlementId(3), second), Some(first));
        assert_eq!(registry.settlements_of(first), vec![SettlementId(1)]);
        assert_eq!(
            registry.settlements_of(second),
            vec![SettlementId(2), SettlementId(3)]
        );
        assert_eq!(registry.stock_capacity(second), 1_000);
    }

    #[test]
    fn only_a_capture_starts_the_conquest_penalty_which_decays_per_turn() {
        let mut registry = NationRegistry::default();
        let first = registry.found(SettlementId(1)).unwrap();
        let second = registry.found(SettlementId(2)).unwrap();
        registry.add_member(SettlementId(3), first);
        assert_eq!(
            registry
                .ownership(SettlementId(3))
                .unwrap()
                .conquest_penalty(),
            0
        );

        assert!(registry.set_under_attack(SettlementId(3), true));
        registry.capture(SettlementId(3), second);
        let ownership = *registry.ownership(SettlementId(3)).unwrap();
        assert_eq!(ownership.conquest_penalty(), 20);
        assert!(ownership.is_freshly_conquered());
        assert!(!ownership.is_under_attack(), "a capture ends the siege");
        for expected in [15, 10, 5, 0, 0] {
            registry.advance_turn();
            assert_eq!(
                registry
                    .ownership(SettlementId(3))
                    .unwrap()
                    .conquest_penalty(),
                expected
            );
        }
        assert!(!registry.set_under_attack(SettlementId(9), true));
    }

    #[test]
    fn removed_nation_ids_are_not_reused() {
        let mut registry = NationRegistry::default();
        let first = registry.found(SettlementId(1)).unwrap();
        let second = registry.found(SettlementId(2)).unwrap();
        registry.capture(SettlementId(1), second);
        assert!(registry.remove(first).is_some());
        assert!(registry.get(first).is_none());
        assert_eq!(registry.found(SettlementId(3)), Some(NationId(2)));
        assert_eq!(registry.len(), 2);
    }
}
