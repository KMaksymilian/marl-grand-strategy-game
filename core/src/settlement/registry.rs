//! One ordered store of persistent settlement instances. Registration is coordinated by World.
use std::collections::{BTreeMap, btree_map::Entry};

use super::Settlement;
use crate::ids::SettlementId;

/// Read-only public access; mutations go through the world's registration/commands.
/// ```compile_fail
/// use simulation_core::settlement::registry::SettlementRegistry;
/// use simulation_core::ids::SettlementId;
/// let mut registry = SettlementRegistry::default();
/// registry.get_mut(SettlementId(1));
/// ```
#[derive(Debug)]
pub struct SettlementRegistry {
    settlements: BTreeMap<SettlementId, Settlement>,
    next_id: Option<SettlementId>,
}

impl Default for SettlementRegistry {
    fn default() -> Self {
        Self {
            settlements: BTreeMap::new(),
            next_id: Some(SettlementId(0)),
        }
    }
}

impl SettlementRegistry {
    pub fn get(&self, id: SettlementId) -> Option<&Settlement> {
        self.settlements.get(&id)
    }

    /// Always ascending by SettlementId, independent of insertion order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Settlement> {
        self.settlements.values()
    }

    pub fn len(&self) -> usize {
        self.settlements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.settlements.is_empty()
    }

    pub(crate) fn get_mut(&mut self, id: SettlementId) -> Option<&mut Settlement> {
        self.settlements.get_mut(&id)
    }

    pub(crate) fn next_id(&self) -> Option<SettlementId> {
        self.next_id
    }

    pub(crate) fn register(&mut self, settlement: Settlement) -> Result<(), SettlementId> {
        let id = settlement.id();
        match self.settlements.entry(id) {
            Entry::Occupied(_) => return Err(id),
            Entry::Vacant(entry) => {
                entry.insert(settlement);
            }
        }
        while let Some(candidate) = self.next_id {
            if !self.settlements.contains_key(&candidate) {
                break;
            }
            self.next_id = candidate.0.checked_add(1).map(SettlementId);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn settlement(id: u32, population: u32) -> Settlement {
        Settlement::new(SettlementId(id), 100, population, Arc::default())
    }

    #[test]
    fn explicit_ids_and_duplicates_preserve_the_registered_instance() {
        let mut registry = SettlementRegistry::default();
        assert!(registry.is_empty());
        for id in [9, 0, 2] {
            registry.register(settlement(id, id)).unwrap();
        }
        assert_eq!(
            registry.iter().map(Settlement::id).collect::<Vec<_>>(),
            vec![SettlementId(0), SettlementId(2), SettlementId(9)]
        );
        assert_eq!(registry.next_id(), Some(SettlementId(1)));
        assert_eq!(registry.register(settlement(2, 99)), Err(SettlementId(2)));
        assert_eq!(registry.get(SettlementId(2)).unwrap().population(), 2);
        assert_eq!(registry.next_id(), Some(SettlementId(1)));
        assert_eq!(registry.len(), 3);
        assert!(registry.get(SettlementId(99)).is_none());
    }

    #[test]
    fn id_allocation_skips_explicit_ids_and_never_wraps() {
        let mut registry = SettlementRegistry::default();
        registry.register(settlement(1, 0)).unwrap();
        registry.register(settlement(0, 0)).unwrap();
        assert_eq!(registry.next_id(), Some(SettlementId(2)));
        // Exercise the overflow boundary without allocating every earlier ID.
        registry.next_id = Some(SettlementId(u32::MAX));
        registry.register(settlement(u32::MAX, 0)).unwrap();
        assert_eq!(registry.next_id(), None);
        assert_eq!(
            registry.get(SettlementId(u32::MAX)).unwrap().id(),
            SettlementId(u32::MAX)
        );
    }
}
