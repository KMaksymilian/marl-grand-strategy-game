//! Troop observations (ADR 0006, MGS-45): armies and settlements see their
//! province and the neighbouring ones and report to their nation (and its
//! allies) at once; caravans see their route and report to the sending
//! settlement on return. Observed absence differs from an unobserved province.
mod common;
use common::material;
use simulation_core::ids::{ArmyId, NationId, SettlementId};
use simulation_core::intelligence::{TroopSighting, TroopSource};
use simulation_core::military::UnitTypeId;
use simulation_core::nation::diplomacy::Treaty;
use simulation_core::settlement::Settlement;
use simulation_core::settlement::decision::{OfferSide, OfferSpec};
use simulation_core::simulation::{
    NationAction, NationDecision, SettlementAction, SettlementDecision, Simulation,
    SimulationTurnReport, TroopObservation,
};
use simulation_core::world::{Map, Province, World, terrain::area::Area};
use std::sync::Arc;

const A: SettlementId = SettlementId(1);
const B: SettlementId = SettlementId(2);
const C: SettlementId = SettlementId(3);
const FIRST: NationId = NationId(0);
const SECOND: NationId = NationId(1);
const THIRD: NationId = NationId(2);

/// Provinces in a row: 10 | 20 | 30 | 40 | 50. A founds nation 0 in 10, B
/// nation 1 in 40 and C nation 2 in 50, 5000 people each; 20 and 30 have no
/// settlement. Every nation holds gold and grain for its armies.
fn simulation() -> Simulation {
    let mut world = World::new(
        Map::new(5, 1, vec![Area::new(0.5, 0.5); 5], vec![10, 20, 30, 40, 50]),
        [10, 20, 30, 40, 50]
            .into_iter()
            .map(Province::new)
            .collect(),
    );
    for (id, province) in [(A, 10), (B, 40), (C, 50)] {
        let mut settlement = Settlement::new(id, 100, 5_000, Arc::default());
        settlement.deposit(material("grain"), 50);
        world.add_settlement(province, settlement).unwrap();
    }
    for nation in [FIRST, SECOND, THIRD] {
        world.deposit_nation_gold(nation, 10_000).unwrap();
        world
            .deposit_nation_resource(nation, material("grain"), 500)
            .unwrap();
    }
    Simulation::new(world).unwrap()
}

fn act(simulation: &mut Simulation, nation: NationId, actions: Vec<NationAction>) {
    simulation
        .queue_nation_decision(NationDecision::new(nation, actions))
        .unwrap();
}

/// Raises two levy units (100 soldiers) for each (nation, settlement) and
/// waits until they are ready; returns the armies in the same order.
fn raise(simulation: &mut Simulation, from: &[(NationId, SettlementId)]) -> Vec<ArmyId> {
    for &(nation, settlement_id) in from {
        act(
            simulation,
            nation,
            vec![NationAction::RecruitUnits {
                settlement_id,
                unit: UnitTypeId::new("levy").unwrap(),
                units: 2,
            }],
        );
    }
    simulation.step().unwrap();
    let report = simulation.step().unwrap();
    from.iter()
        .map(|&(nation, _)| report.nations[&nation].military.recruits_joined[0].army_id)
        .collect()
}

fn march(simulation: &mut Simulation, nation: NationId, army_id: ArmyId, province_id: usize) {
    act(
        simulation,
        nation,
        vec![NationAction::MoveArmy {
            army_id,
            province_id,
        }],
    );
}

fn nation_troops(simulation: &Simulation, nation: NationId) -> Vec<TroopObservation> {
    simulation.observe_nation(nation).unwrap().troops
}

fn find(observations: &[TroopObservation], province: usize) -> Option<&TroopObservation> {
    observations
        .iter()
        .find(|observation| observation.province_id == province)
}

fn provinces(observations: &[TroopObservation]) -> Vec<usize> {
    observations.iter().map(|o| o.province_id).collect()
}

#[test]
fn a_settlement_sees_its_province_and_the_neighbouring_ones() {
    let mut simulation = simulation();
    let report: SimulationTurnReport = simulation.step().unwrap();
    let troops = nation_troops(&simulation, FIRST);
    // 10 and its neighbour 20 are observed; 30 and further were never seen.
    assert_eq!(provinces(&troops), vec![10, 20]);
    for observation in &troops {
        assert_eq!(observation.observed_turn, report.turn);
        assert_eq!(
            observation.source,
            TroopSource::Settlement { settlement_id: A }
        );
        assert!(observation.troops.is_empty(), "an observed absence");
    }
    assert!(find(&troops, 30).is_none(), "unobserved, not empty");
    // The settlement itself knows the same.
    assert_eq!(simulation.observe(A).unwrap().troops, troops);
}

#[test]
fn an_army_sees_neighbouring_provinces_and_reports_to_its_nation_at_once() {
    let mut simulation = simulation();
    let armies = raise(&mut simulation, &[(FIRST, A), (SECOND, B)]);
    march(&mut simulation, FIRST, armies[0], 20);
    march(&mut simulation, SECOND, armies[1], 30);
    let report = simulation.step().unwrap();
    let troops = nation_troops(&simulation, FIRST);
    // From 20, A's army sees 10, 20 and 30: B's 100 soldiers in 30.
    let seen = find(&troops, 30).unwrap();
    assert_eq!(seen.observed_turn, report.turn);
    assert_eq!(seen.source, TroopSource::Army { army_id: armies[0] });
    assert_eq!(
        seen.troops,
        vec![TroopSighting {
            nation_id: SECOND,
            soldiers: 100
        }]
    );
    // Its own army in 20 is no news.
    assert!(find(&troops, 20).unwrap().troops.is_empty());
    assert!(find(&troops, 40).is_none());
    // B sees A's army in 20 from 30, and its own army is left out.
    let theirs = nation_troops(&simulation, SECOND);
    assert_eq!(
        find(&theirs, 20).unwrap().troops,
        vec![TroopSighting {
            nation_id: FIRST,
            soldiers: 100
        }]
    );
    assert!(find(&theirs, 30).unwrap().troops.is_empty());
    // A settlement agent learns from its own watch only: A does not see 30.
    assert!(find(&simulation.observe(A).unwrap().troops, 30).is_none());
}

#[test]
fn an_old_observation_stays_until_a_newer_one_replaces_it() {
    let mut simulation = simulation();
    let armies = raise(&mut simulation, &[(FIRST, A), (SECOND, B)]);
    march(&mut simulation, FIRST, armies[0], 20);
    march(&mut simulation, SECOND, armies[1], 30);
    let seen_in = simulation.step().unwrap().turn;
    // A's army goes home: 30 is out of sight, the knowledge grows old.
    march(&mut simulation, FIRST, armies[0], 10);
    simulation.step().unwrap();
    simulation.step().unwrap();
    let troops = nation_troops(&simulation, FIRST);
    let old = find(&troops, 30).unwrap();
    assert_eq!(old.observed_turn, seen_in);
    assert_eq!(old.troops.len(), 1);
    assert!(
        find(&troops, 20).unwrap().observed_turn > seen_in,
        "20 is still watched"
    );
}

#[test]
fn allies_share_what_their_armies_and_settlements_see() {
    let mut simulation = simulation();
    act(
        &mut simulation,
        FIRST,
        vec![NationAction::ProposeTreaty {
            target: THIRD,
            treaty: Treaty::Alliance,
        }],
    );
    simulation.step().unwrap();
    act(
        &mut simulation,
        THIRD,
        vec![NationAction::RespondToProposal {
            proposer: FIRST,
            treaty: Treaty::Alliance,
            accept: true,
        }],
    );
    simulation.step().unwrap();
    let armies = raise(&mut simulation, &[(SECOND, B), (THIRD, C)]);
    simulation.step().unwrap();
    // C in 50 watches 40 and 50: A learns about B's army in 40 through its
    // ally; its ally's own army in 50 is no news to A.
    let troops = nation_troops(&simulation, FIRST);
    assert_eq!(
        find(&troops, 40).unwrap().troops,
        vec![TroopSighting {
            nation_id: SECOND,
            soldiers: 100
        }]
    );
    assert_eq!(
        find(&troops, 40).unwrap().source,
        TroopSource::Settlement { settlement_id: C }
    );
    assert!(find(&troops, 50).unwrap().troops.is_empty());
    let _ = armies;
    // B has no ally: it knows nothing about 10.
    assert!(find(&nation_troops(&simulation, SECOND), 10).is_none());
}

#[test]
fn a_caravan_sees_its_route_and_tells_its_settlement_on_return() {
    let mut simulation = simulation();
    let armies = raise(&mut simulation, &[(SECOND, B)]);
    march(&mut simulation, SECOND, armies[0], 30);
    simulation.step().unwrap();
    // A sends a caravan to C in 50 through 20, 30 and 40.
    simulation
        .queue_decision(SettlementDecision::new(
            A,
            vec![SettlementAction::OrderCaravan {
                receiver_id: C,
                offers: vec![OfferSpec {
                    resource: material("grain"),
                    side: OfferSide::Sell,
                    units: 5,
                    price_per_unit: 1,
                }],
            }],
        ))
        .unwrap();
    let ordered = simulation.step().unwrap().turn;
    let mut delivered = None;
    for _ in 0..10 {
        let report = simulation.step().unwrap();
        let known = simulation.observe(A).unwrap().troops;
        if let Some(seen) = find(&known, 30).cloned() {
            delivered = Some((report.turn, seen));
            break;
        }
    }
    let (delivered_in, seen) = delivered.expect("the caravan came back");
    assert_eq!(seen.source, TroopSource::Caravan { sender_id: A });
    assert_eq!(
        seen.troops,
        vec![TroopSighting {
            nation_id: SECOND,
            soldiers: 100
        }]
    );
    assert!(
        seen.observed_turn > ordered && seen.observed_turn < delivered_in,
        "seen on the way, known only after the return"
    );
    let known = simulation.observe(A).unwrap().troops;
    assert!(find(&known, 40).is_some() && find(&known, 50).is_some());
    // The caravan reports to its settlement, not to the nation.
    assert!(find(&nation_troops(&simulation, FIRST), 30).is_none());
}

#[test]
fn troop_observations_serialize_with_their_source() {
    let mut simulation = simulation();
    simulation.step().unwrap();
    let observation = simulation.observe_nation(FIRST).unwrap();
    let json = serde_json::to_value(&observation.troops[0]).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"province_id": 10, "observed_turn": 1,
            "source": {"observer": "settlement", "settlement_id": 1}, "troops": []})
    );
    assert_eq!(
        serde_json::from_str::<simulation_core::simulation::NationObservation>(
            &observation.to_json()
        )
        .unwrap(),
        observation
    );
}
