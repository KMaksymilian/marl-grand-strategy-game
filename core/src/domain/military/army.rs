//! Armies of nations (gameplay specification §10): units, armies and the
//! recruitments in progress.
//!
//! A unit (oddział) starts with `soldiers_per_unit` people taken from one
//! settlement; battles kill some of them, so each unit counts its living
//! soldiers. An army is a group of one nation's units standing in one
//! province; it occupies the whole province. Recruited soldiers leave their
//! settlement for good: disbanding returns the living ones to the settlement
//! of the province where the army stands.
use super::UnitTypeId;
use crate::domain::ids::{ArmyId, NationId, SettlementId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One unit of an army.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unit {
    pub unit_type: UnitTypeId,
    /// Living soldiers; the upkeep is paid per soldier.
    pub soldiers: u32,
    /// From 0 to the maximum; a unit at 0 deserts (§10.3).
    pub morale: u32,
    /// The settlement the soldiers came from, for its conscription pool.
    pub raised_in: SettlementId,
}

/// A group of one nation's units in one province (§10.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Army {
    id: ArmyId,
    nation: NationId,
    province: usize,
    units: Vec<Unit>,
    /// The province the army marches to; None when it stays (§11.1).
    destination: Option<usize>,
    /// The province the army entered its current one from, where it retreats
    /// after a lost attack (§11.3); None for an army raised where it stands.
    came_from: Option<usize>,
}

impl Army {
    pub fn id(&self) -> ArmyId {
        self.id
    }

    pub fn nation(&self) -> NationId {
        self.nation
    }

    /// The province the army occupies.
    pub fn province(&self) -> usize {
        self.province
    }

    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    pub fn soldiers(&self) -> u32 {
        self.units.iter().map(|unit| unit.soldiers).sum()
    }

    pub(crate) fn units_mut(&mut self) -> &mut Vec<Unit> {
        &mut self.units
    }

    pub fn destination(&self) -> Option<usize> {
        self.destination
    }

    pub fn came_from(&self) -> Option<usize> {
        self.came_from
    }

    pub(crate) fn set_destination(&mut self, destination: Option<usize>) {
        self.destination = destination;
    }

    /// Enters a neighbouring province by marching: remembers where it came from.
    pub(crate) fn enter(&mut self, province: usize) {
        self.came_from = Some(self.province);
        self.province = province;
    }

    /// Placed in a province without marching (retreat, withdrawal): there is
    /// nothing to retreat to from there and no march continues.
    pub(crate) fn relocate(&mut self, province: usize) {
        self.province = province;
        self.came_from = None;
        self.destination = None;
    }
}

/// A settlement's garrison (§11.4): levy from a share of its people, without
/// upkeep and outside the conscription pool. Its soldiers stay part of the
/// population; the fallen leave it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Garrison {
    pub soldiers: u32,
    pub morale: u32,
}

/// Units ordered by a nation that are not ready yet (§10.2). Their soldiers
/// and equipment are already taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recruitment {
    pub nation: NationId,
    pub settlement: SettlementId,
    pub unit_type: UnitTypeId,
    pub units: u32,
    /// Soldiers per unit at recruitment.
    pub soldiers_per_unit: u32,
    /// Turns until the units join an army; they are ready at the end of the
    /// turn in which this reaches zero.
    pub turns_left: u32,
}

/// Every army and recruitment of the game.
#[derive(Debug, Default)]
pub struct ArmyRegistry {
    armies: BTreeMap<ArmyId, Army>,
    recruitments: Vec<Recruitment>,
    /// Garrisons changed by battles; a settlement missing here has a full one.
    garrisons: BTreeMap<SettlementId, Garrison>,
    next_id: u32,
}

impl ArmyRegistry {
    /// Ascending ID.
    pub fn iter(&self) -> impl Iterator<Item = &Army> {
        self.armies.values()
    }

    pub fn get(&self, id: ArmyId) -> Option<&Army> {
        self.armies.get(&id)
    }

    pub(crate) fn get_mut(&mut self, id: ArmyId) -> Option<&mut Army> {
        self.armies.get_mut(&id)
    }

    /// Armies of one nation, ascending ID.
    pub fn of_nation(&self, nation: NationId) -> impl Iterator<Item = &Army> {
        self.armies
            .values()
            .filter(move |army| army.nation == nation)
    }

    pub fn recruitments(&self) -> &[Recruitment] {
        &self.recruitments
    }

    /// Soldiers raised in a settlement who still serve or are being
    /// recruited: the used part of its conscription pool.
    pub fn serving_from(&self, settlement: SettlementId) -> u32 {
        let in_armies: u32 = self
            .armies
            .values()
            .flat_map(|army| &army.units)
            .filter(|unit| unit.raised_in == settlement)
            .map(|unit| unit.soldiers)
            .sum();
        let recruiting: u32 = self
            .recruitments
            .iter()
            .filter(|recruitment| recruitment.settlement == settlement)
            .map(|recruitment| recruitment.units * recruitment.soldiers_per_unit)
            .sum();
        in_armies + recruiting
    }

    /// Whether `station` would succeed: the nation has an army in the
    /// province, or a new army ID is left.
    pub(crate) fn can_station(&self, nation: NationId, province: usize) -> bool {
        self.next_id < u32::MAX
            || self
                .armies
                .values()
                .any(|army| army.nation == nation && army.province == province)
    }

    /// Adds units to the nation's army with the lowest ID in the province, or
    /// to a new army there. Returns the army.
    pub(crate) fn station(
        &mut self,
        nation: NationId,
        province: usize,
        units: Vec<Unit>,
    ) -> Option<ArmyId> {
        let existing = self
            .armies
            .values()
            .find(|army| army.nation == nation && army.province == province)
            .map(|army| army.id);
        let id = match existing {
            Some(id) => id,
            None => self.create(nation, province)?,
        };
        self.armies
            .get_mut(&id)
            .expect("found or created above")
            .units
            .extend(units);
        Some(id)
    }

    /// A new empty army; None when the IDs are exhausted.
    pub(crate) fn create(&mut self, nation: NationId, province: usize) -> Option<ArmyId> {
        let id = ArmyId(self.next_id);
        self.next_id = self.next_id.checked_add(1)?;
        self.armies.insert(
            id,
            Army {
                id,
                nation,
                province,
                units: Vec::new(),
                destination: None,
                came_from: None,
            },
        );
        Some(id)
    }

    pub(crate) fn remove(&mut self, id: ArmyId) -> Option<Army> {
        self.armies.remove(&id)
    }

    pub(crate) fn add_recruitment(&mut self, recruitment: Recruitment) {
        self.recruitments.push(recruitment);
    }

    /// End of a turn: every recruitment counts down; those at zero leave the
    /// list and are returned to be stationed.
    pub(crate) fn advance_recruitments(&mut self) -> Vec<Recruitment> {
        for recruitment in &mut self.recruitments {
            recruitment.turns_left = recruitment.turns_left.saturating_sub(1);
        }
        let (ready, waiting) = std::mem::take(&mut self.recruitments)
            .into_iter()
            .partition(|recruitment| recruitment.turns_left == 0);
        self.recruitments = waiting;
        ready
    }

    /// Removes armies without units.
    pub(crate) fn remove_empty(&mut self) {
        self.armies.retain(|_, army| !army.units.is_empty());
    }

    /// The garrison after battles; None when it is full.
    pub fn garrison(&self, settlement: SettlementId) -> Option<Garrison> {
        self.garrisons.get(&settlement).copied()
    }

    pub(crate) fn set_garrison(&mut self, settlement: SettlementId, garrison: Garrison) {
        self.garrisons.insert(settlement, garrison);
    }

    /// The garrison is full again (it refilled while not attacked).
    pub(crate) fn reset_garrison(&mut self, settlement: SettlementId) {
        self.garrisons.remove(&settlement);
    }

    /// A nation that left the game takes its armies and recruitments with it.
    pub(crate) fn remove_nation(&mut self, nation: NationId) {
        self.armies.retain(|_, army| army.nation != nation);
        self.recruitments
            .retain(|recruitment| recruitment.nation != nation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(settlement: u32, soldiers: u32) -> Unit {
        Unit {
            unit_type: UnitTypeId::new("levy").unwrap(),
            soldiers,
            morale: 100,
            raised_in: SettlementId(settlement),
        }
    }

    #[test]
    fn units_join_the_nations_army_in_the_province_or_a_new_one() {
        let mut registry = ArmyRegistry::default();
        let first = registry
            .station(NationId(0), 10, vec![unit(1, 50)])
            .unwrap();
        let same = registry
            .station(NationId(0), 10, vec![unit(1, 50)])
            .unwrap();
        let other_nation = registry
            .station(NationId(1), 10, vec![unit(2, 50)])
            .unwrap();
        let other_province = registry
            .station(NationId(0), 20, vec![unit(1, 50)])
            .unwrap();
        assert_eq!(first, same);
        assert_ne!(first, other_nation);
        assert_ne!(first, other_province);
        assert_eq!(registry.get(first).unwrap().soldiers(), 100);
        assert_eq!(registry.of_nation(NationId(0)).count(), 2);
    }

    #[test]
    fn the_pool_counts_serving_and_recruiting_soldiers_of_one_settlement() {
        let mut registry = ArmyRegistry::default();
        registry.station(NationId(0), 10, vec![unit(1, 30), unit(2, 50)]);
        registry.add_recruitment(Recruitment {
            nation: NationId(0),
            settlement: SettlementId(1),
            unit_type: UnitTypeId::new("archers").unwrap(),
            units: 2,
            soldiers_per_unit: 50,
            turns_left: 2,
        });
        assert_eq!(registry.serving_from(SettlementId(1)), 130);
        assert_eq!(registry.serving_from(SettlementId(2)), 50);
        assert!(registry.advance_recruitments().is_empty());
        assert_eq!(registry.advance_recruitments().len(), 1);
        assert!(registry.recruitments().is_empty());
    }
}
