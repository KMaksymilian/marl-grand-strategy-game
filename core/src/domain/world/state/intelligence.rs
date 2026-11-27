//! Troop observations of a turn (ADR 0006, MGS-45).
//!
//! At the end of every turn, at the final positions of the armies:
//!
//! - every army sees its province and the neighbouring ones; its nation and
//!   the nation's allies know at once;
//! - every settlement sees its province and the neighbouring ones; it, its
//!   nation and the nation's allies know at once;
//! - every caravan sees the provinces it passed this turn; it carries the
//!   observations home and its sending settlement knows them on its return,
//!   with the turn they were made.
//!
//! An observation records every army in the province, including the
//! observer's own; readers leave out their own and their allies' armies.
use super::World;
use crate::domain::ids::NationId;
use crate::domain::intelligence::{Information, InformationSnapshot, TroopSighting, TroopSource};
use crate::domain::nation::diplomacy::Relation;
use crate::domain::world::Position;
use std::collections::BTreeMap;

impl World {
    /// Every nation's living soldiers in the province, ascending nation ID.
    fn troops_in(&self, province: usize) -> Vec<TroopSighting> {
        let mut soldiers: BTreeMap<NationId, u32> = BTreeMap::new();
        for army in self
            .armies
            .iter()
            .filter(|army| army.province() == province)
        {
            *soldiers.entry(army.nation()).or_default() += army.soldiers();
        }
        soldiers
            .into_iter()
            .map(|(nation_id, soldiers)| TroopSighting {
                nation_id,
                soldiers,
            })
            .collect()
    }

    /// A province and its neighbours, ascending ID.
    fn watched_from(&self, province: usize) -> Vec<usize> {
        let mut provinces: Vec<_> = std::iter::once(province)
            .chain(self.province_graph.neighbors(province))
            .collect();
        provinces.sort_unstable();
        provinces
    }

    fn sighting(&self, turn: u64, province_id: usize, source: TroopSource) -> InformationSnapshot {
        InformationSnapshot::new(
            turn,
            Information::TroopPresence {
                province_id,
                source,
                troops: self.troops_in(province_id),
            },
        )
    }

    /// The nation and its allies: who learns what its armies and settlements see.
    fn sharing_with(&self, nation: NationId) -> Vec<NationId> {
        self.nations
            .iter()
            .map(|other| other.id())
            .filter(|&other| {
                other == nation || self.diplomacy.relation(nation, other) == Relation::Alliance
            })
            .collect()
    }

    /// The troop observations of this turn (see the module documentation).
    pub(crate) fn observe_troops(&mut self) {
        let turn = self.current_turn();
        let mut for_nations: Vec<(NationId, InformationSnapshot)> = Vec::new();

        let armies: Vec<_> = self
            .armies
            .iter()
            .map(|army| (army.id(), army.nation(), army.province()))
            .collect();
        for (army_id, nation, province) in armies {
            for watched in self.watched_from(province) {
                let snapshot = self.sighting(turn, watched, TroopSource::Army { army_id });
                for recipient in self.sharing_with(nation) {
                    for_nations.push((recipient, snapshot.clone()));
                }
            }
        }

        for settlement_id in self.settlement_ids() {
            let (Some(owner), Some(province)) = (
                self.settlement_owner(settlement_id),
                self.settlement_province(settlement_id),
            ) else {
                continue;
            };
            for watched in self.watched_from(province) {
                let snapshot =
                    self.sighting(turn, watched, TroopSource::Settlement { settlement_id });
                for recipient in self.sharing_with(owner) {
                    for_nations.push((recipient, snapshot.clone()));
                }
                self.settlement_mut(settlement_id)
                    .expect("listed above")
                    .remember_troop_observation(snapshot);
            }
        }

        for (nation, snapshot) in for_nations {
            if let Ok(nation) = self.nation_mut(nation) {
                nation.remember_troops(snapshot);
            }
        }

        // Caravans: the provinces of the tiles passed this turn, in order.
        for index in 0..self.caravans.len() {
            let passed = self.caravans[index].take_passed();
            let sender_id = self.caravans[index].sender_id;
            let mut provinces: Vec<usize> = Vec::new();
            for tile in passed {
                let position = Position {
                    x: tile.0 as i32,
                    y: tile.1 as i32,
                };
                if let Some(province) = self.map.province_at(position)
                    && !provinces.contains(&province)
                {
                    provinces.push(province);
                }
            }
            for province in provinces {
                let snapshot = self.sighting(turn, province, TroopSource::Caravan { sender_id });
                self.caravans[index].record_observation(snapshot);
            }
        }
    }
}
