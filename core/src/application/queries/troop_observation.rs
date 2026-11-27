//! What a nation or a settlement knows about troops (ADR 0006, MGS-45): the
//! latest observation of each province, with the turn and the observer.
//!
//! Stored observations record every army in a province. Reading them, the
//! recipient's own armies and its allies' are left out (by the relations of
//! now): only foreign troops are news. An entry with no troops is an observed
//! absence; a province without an entry was never observed.
use crate::domain::ids::NationId;
use crate::domain::intelligence::{Information, InformationSnapshot, TroopSighting, TroopSource};
use crate::domain::nation::diplomacy::Relation;
use crate::domain::world::World;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TroopObservation {
    pub province_id: usize,
    /// The turn the province was seen, not the turn the news arrived.
    pub observed_turn: u64,
    pub source: TroopSource,
    /// Foreign armies seen there, one entry per nation, ascending nation ID;
    /// empty: no foreign army stood there.
    pub troops: Vec<TroopSighting>,
}

/// The stored snapshots as seen by `viewer` (None: hide nothing).
pub(crate) fn troop_observations<'a>(
    world: &World,
    viewer: Option<NationId>,
    snapshots: impl Iterator<Item = &'a InformationSnapshot>,
) -> Vec<TroopObservation> {
    let friendly = |nation: NationId| {
        viewer.is_some_and(|viewer| {
            nation == viewer || world.diplomacy().relation(viewer, nation) == Relation::Alliance
        })
    };
    let mut observations: Vec<_> = snapshots
        .filter_map(|snapshot| match snapshot.information() {
            Information::TroopPresence {
                province_id,
                source,
                troops,
            } => Some(TroopObservation {
                province_id: *province_id,
                observed_turn: snapshot.observed_turn(),
                source: *source,
                troops: troops
                    .iter()
                    .filter(|sighting| !friendly(sighting.nation_id))
                    .copied()
                    .collect(),
            }),
            Information::ForeignMarket { .. } => None,
        })
        .collect();
    observations.sort_by_key(|observation| observation.province_id);
    observations
}
