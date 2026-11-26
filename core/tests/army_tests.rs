//! Armies (gameplay specification §10): recruitment from settlement
//! populations with the conscription pool, equipment from the national stock,
//! the recruitment time and Mobilization, upkeep from the national stock and
//! treasury shared equally, morale and desertion, grouping and disbanding.
mod common;
use common::material;
use simulation_core::ids::{ArmyId, NationId, SettlementId};
use simulation_core::military::{RecruitsJoined, UnitTypeId};
use simulation_core::nation::decree::DecreeId;
use simulation_core::settlement::Settlement;
use simulation_core::simulation::{
    NationAction, NationActionEffect, NationActionError, NationActionOutcome, NationDecision,
    NationObservation, Simulation, SimulationTurnReport,
};
use simulation_core::world::{Conscription, Map, Province, World, terrain::area::Area};
use std::sync::Arc;

const A: SettlementId = SettlementId(1);
const B: SettlementId = SettlementId(2);
const C: SettlementId = SettlementId(3);
const FIRST: NationId = NationId(0);
const SECOND: NationId = NationId(1);

/// Provinces 10, 20, 30 in a row with 1000 people each. A founds nation 0
/// and B joins it; C founds nation 1. Nation 0 holds 10 000 gold.
fn world() -> World {
    let mut world = World::new(
        Map::new(3, 1, vec![Area::new(0.5, 0.5); 3], vec![10, 20, 30]),
        [10, 20, 30].into_iter().map(Province::new).collect(),
    );
    world
        .add_settlement(10, Settlement::new(A, 100, 1_000, Arc::default()))
        .unwrap();
    world
        .add_settlement_to_nation(FIRST, 20, Settlement::new(B, 100, 1_000, Arc::default()))
        .unwrap();
    world
        .add_settlement(30, Settlement::new(C, 100, 1_000, Arc::default()))
        .unwrap();
    world.deposit_nation_gold(FIRST, 10_000).unwrap();
    world
}

fn simulation() -> Simulation {
    Simulation::new(world()).unwrap()
}

fn unit(id: &str) -> UnitTypeId {
    UnitTypeId::new(id).unwrap()
}

fn recruit(settlement_id: SettlementId, unit_type: &str, units: u32) -> NationAction {
    NationAction::RecruitUnits {
        settlement_id,
        unit: unit(unit_type),
        units,
    }
}

fn act(simulation: &mut Simulation, actions: Vec<NationAction>) -> SimulationTurnReport {
    simulation
        .queue_nation_decision(NationDecision::new(FIRST, actions))
        .unwrap();
    simulation.step().unwrap()
}

fn outcomes(report: &SimulationTurnReport) -> Vec<NationActionOutcome> {
    report
        .nation_decisions
        .iter()
        .rfind(|decision| decision.nation_id == FIRST)
        .map(|decision| decision.results.iter().map(|r| r.outcome.clone()).collect())
        .unwrap_or_default()
}

fn error(outcome: &NationActionOutcome) -> NationActionError {
    match outcome {
        NationActionOutcome::Rejected { error } => error.clone(),
        NationActionOutcome::Applied { effect } => panic!("applied: {effect:?}"),
    }
}

fn effect(outcome: &NationActionOutcome) -> NationActionEffect {
    match outcome {
        NationActionOutcome::Applied { effect } => effect.clone(),
        NationActionOutcome::Rejected { error } => panic!("rejected: {error:?}"),
    }
}

fn observe(simulation: &Simulation) -> NationObservation {
    simulation.observe_nation(FIRST).unwrap()
}

fn population(simulation: &Simulation, id: SettlementId) -> u32 {
    simulation.world().settlement(id).unwrap().population()
}

fn stock(simulation: &Simulation, resource: &str) -> u32 {
    simulation
        .world()
        .nation(FIRST)
        .unwrap()
        .stock()
        .amount(material(resource))
}

fn treasury(simulation: &Simulation) -> u64 {
    simulation.world().nation(FIRST).unwrap().treasury()
}

/// Recruits two levy units in A (100 soldiers) and lets them become ready.
fn army_of_two_levies(simulation: &mut Simulation) -> ArmyId {
    act(simulation, vec![recruit(A, "levy", 2)]);
    let report = simulation.step().unwrap();
    report.nations[&FIRST].military.recruits_joined[0].army_id
}

// ---------------------------------------------------------------- recruitment

#[test]
fn recruitment_takes_people_now_and_the_units_are_ready_after_two_turns() {
    let mut simulation = simulation();
    let report = act(&mut simulation, vec![recruit(A, "levy", 2)]);
    assert_eq!(
        effect(&outcomes(&report)[0]),
        NationActionEffect::UnitsRecruited {
            settlement_id: A,
            unit: unit("levy"),
            units: 2,
            soldiers: 100,
            ready_in_turns: 2,
            army_id: None,
        }
    );
    // The people left at once and work no more; no army yet.
    assert_eq!(population(&simulation, A), 900);
    assert!(simulation.world().armies().iter().next().is_none());
    let observation = observe(&simulation);
    assert_eq!(observation.military.recruitments.len(), 1);
    assert_eq!(observation.military.recruitments[0].turns_left, 1);

    let report = simulation.step().unwrap();
    let joined = &report.nations[&FIRST].military.recruits_joined;
    assert_eq!(
        joined,
        &vec![RecruitsJoined {
            army_id: joined[0].army_id,
            settlement_id: A,
            unit_type: unit("levy"),
            units: 2,
        }]
    );
    let army = simulation.world().army(joined[0].army_id).unwrap();
    assert_eq!(army.province(), 10, "the settlement's province");
    assert_eq!(army.nation(), FIRST);
    assert_eq!(army.soldiers(), 100);
    assert!(army.units().iter().all(|unit| unit.morale == 100));
    assert!(observe(&simulation).military.recruitments.is_empty());
}

#[test]
fn the_conscription_pool_is_ten_percent_of_the_people_and_counts_serving_soldiers() {
    let mut simulation = simulation();
    assert_eq!(
        observe(&simulation).military.conscription[&A],
        Conscription {
            serving: 0,
            limit: 100,
            available: 100
        }
    );
    let report = act(
        &mut simulation,
        vec![
            recruit(A, "levy", 3),
            recruit(A, "levy", 2),
            recruit(A, "levy", 1),
        ],
    );
    let results = outcomes(&report);
    assert_eq!(
        error(&results[0]),
        NationActionError::ConscriptionPoolExceeded {
            settlement_id: A,
            requested: 150,
            available: 100
        }
    );
    assert!(matches!(
        effect(&results[1]),
        NationActionEffect::UnitsRecruited { soldiers: 100, .. }
    ));
    // The 100 recruits still count: 10% of 900 + 100.
    assert_eq!(
        error(&results[2]),
        NationActionError::ConscriptionPoolExceeded {
            settlement_id: A,
            requested: 50,
            available: 0
        }
    );
    // The pool of each settlement is its own.
    let report = act(&mut simulation, vec![recruit(B, "levy", 2)]);
    assert!(matches!(
        effect(&outcomes(&report)[0]),
        NationActionEffect::UnitsRecruited { .. }
    ));
}

#[test]
fn recruitment_is_checked_before_anything_is_taken() {
    let mut simulation = simulation();
    let report = act(
        &mut simulation,
        vec![
            recruit(A, "knights", 1),
            recruit(A, "levy", 0),
            recruit(C, "levy", 1),
            recruit(SettlementId(9), "levy", 1),
            recruit(A, "spearmen", 1),
        ],
    );
    let errors: Vec<_> = outcomes(&report).iter().map(error).collect();
    assert_eq!(
        errors,
        vec![
            NationActionError::UnknownUnitType {
                unit: unit("knights")
            },
            NationActionError::EmptyRecruitment,
            NationActionError::SettlementNotOwned { settlement_id: C },
            NationActionError::UnknownSettlement {
                settlement_id: SettlementId(9)
            },
            // 25 bronze and 25 planks per unit, none in the national stock;
            // the first missing material in catalogue order is reported.
            NationActionError::InsufficientStock {
                resource: material("planks"),
                requested: 25,
                available: 0
            },
        ]
    );
    assert_eq!(population(&simulation, A), 1_000, "nothing was taken");
}

#[test]
fn equipment_comes_from_the_national_stock() {
    let mut world = world();
    for (resource, amount) in [("bronze", 60), ("planks", 50)] {
        world
            .deposit_nation_resource(FIRST, material(resource), amount)
            .unwrap();
    }
    let mut simulation = Simulation::new(world).unwrap();
    let report = act(&mut simulation, vec![recruit(A, "spearmen", 2)]);
    assert!(matches!(
        effect(&outcomes(&report)[0]),
        NationActionEffect::UnitsRecruited { soldiers: 100, .. }
    ));
    assert_eq!(stock(&simulation, "bronze"), 10);
    assert_eq!(stock(&simulation, "planks"), 0);
}

#[test]
fn mobilization_doubles_the_pool_and_makes_recruitment_instant() {
    let mut simulation = simulation();
    act(
        &mut simulation,
        vec![NationAction::EnactDecree {
            decree: DecreeId::new("mobilization").unwrap(),
        }],
    );
    // Active from this turn: 20% of 1000 and no waiting.
    let report = act(&mut simulation, vec![recruit(A, "levy", 4)]);
    let NationActionEffect::UnitsRecruited {
        soldiers,
        ready_in_turns,
        army_id,
        ..
    } = effect(&outcomes(&report)[0])
    else {
        panic!("not a recruitment");
    };
    assert_eq!((soldiers, ready_in_turns), (200, 0));
    let army = simulation.world().army(army_id.unwrap()).unwrap();
    assert_eq!(army.soldiers(), 200);
    assert_eq!(
        observe(&simulation).military.conscription[&A],
        Conscription {
            serving: 200,
            limit: 200,
            available: 0
        }
    );
}

#[test]
fn units_of_a_lost_settlement_join_at_the_capital() {
    let mut simulation = simulation();
    act(&mut simulation, vec![recruit(B, "levy", 1)]);
    simulation.transfer_settlement(B, SECOND).unwrap();
    let report = simulation.step().unwrap();
    let army = report.nations[&FIRST].military.recruits_joined[0].army_id;
    assert_eq!(simulation.world().army(army).unwrap().province(), 10);
}

// ---------------------------------------------------------------- upkeep and morale

#[test]
fn full_upkeep_raises_morale_partial_upkeep_lowers_it_and_zero_morale_deserts() {
    let mut simulation = simulation();
    let army = army_of_two_levies(&mut simulation);
    let morale = |simulation: &Simulation| {
        simulation
            .world()
            .army(army)
            .map(|army| army.units()[0].morale)
    };
    // No food: −20 for every unit; the gold is taken anyway.
    let gold_before = treasury(&simulation);
    let report = simulation.step().unwrap();
    let upkeep = &report.nations[&FIRST].military.upkeep;
    assert_eq!(
        (upkeep.soldiers, upkeep.food_demand, upkeep.gold_demand),
        (100, 100, 100)
    );
    assert_eq!((upkeep.food_paid, upkeep.gold_paid), (0, 100));
    assert!(!upkeep.fully_supplied);
    assert_eq!(upkeep.morale_change, -20);
    assert_eq!(treasury(&simulation), gold_before - 100);
    assert_eq!(morale(&simulation), Some(80));
    simulation.step().unwrap();
    assert_eq!(morale(&simulation), Some(60));

    // Raw food in full: +20.
    let mut world = simulation.into_world();
    world
        .deposit_nation_resource(FIRST, material("grain"), 100)
        .unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(report.nations[&FIRST].military.upkeep.morale_change, 20);
    assert_eq!(morale(&simulation), Some(80));

    // Tier-2 food in full: +30, up to the maximum.
    let mut world = simulation.into_world();
    world
        .deposit_nation_resource(FIRST, material("bread"), 100)
        .unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    let report = simulation.step().unwrap();
    let upkeep = &report.nations[&FIRST].military.upkeep;
    assert_eq!((upkeep.tier2_food_paid, upkeep.morale_change), (100, 30));
    assert_eq!(morale(&simulation), Some(100));

    // Five unfed turns: the units desert and the army is gone.
    let mut deserted = Vec::new();
    for _ in 0..5 {
        let report = simulation.step().unwrap();
        deserted.extend(report.nations[&FIRST].military.upkeep.deserted.clone());
    }
    assert_eq!(deserted.len(), 2);
    assert!(deserted.iter().all(|unit| unit.soldiers == 50));
    assert_eq!(morale(&simulation), None);
    assert!(observe(&simulation).military.armies.is_empty());
}

#[test]
fn a_shortage_is_shared_equally_so_every_unit_loses_morale() {
    let mut world = world();
    world
        .deposit_nation_resource(FIRST, material("grain"), 150)
        .unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    // Two armies of 100 soldiers in provinces 10 and 20: 200 food needed.
    act(
        &mut simulation,
        vec![recruit(A, "levy", 2), recruit(B, "levy", 2)],
    );
    simulation.step().unwrap();
    let report = simulation.step().unwrap();
    let upkeep = &report.nations[&FIRST].military.upkeep;
    assert_eq!((upkeep.food_demand, upkeep.food_paid), (200, 150));
    assert_eq!(
        stock(&simulation, "grain"),
        0,
        "all that was there is eaten"
    );
    for army in simulation.world().armies().iter() {
        assert!(army.units().iter().all(|unit| unit.morale == 80));
    }
}

#[test]
fn discipline_halves_morale_losses_and_raises_the_gold_upkeep() {
    let mut simulation = simulation();
    let army = army_of_two_levies(&mut simulation);
    act(
        &mut simulation,
        vec![NationAction::EnactDecree {
            decree: DecreeId::new("discipline").unwrap(),
        }],
    );
    let report = simulation.step().unwrap();
    let upkeep = &report.nations[&FIRST].military.upkeep;
    assert_eq!(upkeep.gold_demand, 125, "100 gold ×1.25");
    assert_eq!(upkeep.morale_change, -10, "unfed: −20 ×0.5");
    let morale = simulation.world().army(army).unwrap().units()[0].morale;
    assert_eq!(morale, 70, "−20 in the turn of enactment, then −10");
}

#[test]
fn cavalry_and_siege_engines_cost_two_gold_per_soldier() {
    let mut world = world();
    for (resource, amount) in [("cattle", 25), ("steel", 25), ("planks", 50), ("tools", 25)] {
        world
            .deposit_nation_resource(FIRST, material(resource), amount)
            .unwrap();
    }
    let mut simulation = Simulation::new(world).unwrap();
    act(
        &mut simulation,
        vec![recruit(A, "cavalry", 1), recruit(A, "siege_engines", 1)],
    );
    simulation.step().unwrap();
    assert_eq!(observe(&simulation).military.gold_upkeep, 200);
    assert_eq!(observe(&simulation).military.food_upkeep, 100);
}

// ---------------------------------------------------------------- grouping and disbanding

#[test]
fn armies_split_and_merge_within_one_province() {
    let mut simulation = simulation();
    let army = army_of_two_levies(&mut simulation);
    let report = act(
        &mut simulation,
        vec![NationAction::SplitArmy {
            army_id: army,
            units: vec![1],
        }],
    );
    let NationActionEffect::ArmySplit { new_army_id, .. } = effect(&outcomes(&report)[0]) else {
        panic!("not a split");
    };
    let armies = observe(&simulation).military.armies;
    assert_eq!(armies.len(), 2);
    assert!(
        armies
            .iter()
            .all(|army| army.soldiers == 50 && army.province_id == 10)
    );
    assert!(armies.iter().all(|army| army.movement == 1));

    let report = act(
        &mut simulation,
        vec![
            NationAction::MergeArmies {
                army_id: army,
                into: army,
            },
            NationAction::SplitArmy {
                army_id: army,
                units: vec![0],
            },
            NationAction::MergeArmies {
                army_id: new_army_id,
                into: army,
            },
            NationAction::DisbandArmy {
                army_id: new_army_id,
            },
        ],
    );
    let results = outcomes(&report);
    assert_eq!(
        error(&results[0]),
        NationActionError::SameArmy { army_id: army }
    );
    assert_eq!(
        error(&results[1]),
        NationActionError::InvalidUnitSelection { army_id: army },
        "at least one unit stays"
    );
    assert!(matches!(
        effect(&results[2]),
        NationActionEffect::ArmiesMerged { .. }
    ));
    assert_eq!(
        error(&results[3]),
        NationActionError::UnknownArmy {
            army_id: new_army_id
        },
        "the merged army is gone"
    );
    assert_eq!(simulation.world().army(army).unwrap().soldiers(), 100);
}

#[test]
fn armies_in_different_provinces_or_of_other_nations_cannot_be_merged() {
    let mut simulation = simulation();
    act(
        &mut simulation,
        vec![recruit(A, "levy", 1), recruit(B, "levy", 1)],
    );
    let report = simulation.step().unwrap();
    let joined = &report.nations[&FIRST].military.recruits_joined;
    let (in_a, in_b) = (joined[0].army_id, joined[1].army_id);
    let report = act(
        &mut simulation,
        vec![NationAction::MergeArmies {
            army_id: in_a,
            into: in_b,
        }],
    );
    assert_eq!(
        error(&outcomes(&report)[0]),
        NationActionError::ArmiesApart {
            army_id: in_a,
            into: in_b
        }
    );
    simulation
        .queue_nation_decision(NationDecision::new(
            SECOND,
            vec![NationAction::DisbandArmy { army_id: in_a }],
        ))
        .unwrap();
    let report = simulation.step().unwrap();
    let theirs = &report
        .nation_decisions
        .iter()
        .find(|decision| decision.nation_id == SECOND)
        .unwrap()
        .results[0]
        .outcome;
    assert_eq!(
        error(theirs),
        NationActionError::ArmyNotOwned { army_id: in_a }
    );
}

#[test]
fn disbanding_returns_the_soldiers_to_the_settlement_where_the_army_stands() {
    let mut simulation = simulation();
    let army = army_of_two_levies(&mut simulation);
    assert_eq!(population(&simulation, A), 900);
    let report = act(
        &mut simulation,
        vec![NationAction::DisbandArmy { army_id: army }],
    );
    assert_eq!(
        effect(&outcomes(&report)[0]),
        NationActionEffect::ArmyDisbanded {
            army_id: army,
            settlement_id: A,
            soldiers: 100
        }
    );
    assert_eq!(population(&simulation, A), 1_000);
    assert!(simulation.world().army(army).is_none());

    // In a province that is no longer own, the army cannot be disbanded.
    let army = army_of_two_levies(&mut simulation);
    simulation.transfer_settlement(A, SECOND).unwrap();
    let report = act(
        &mut simulation,
        vec![NationAction::DisbandArmy { army_id: army }],
    );
    assert_eq!(
        error(&outcomes(&report)[0]),
        NationActionError::NotOwnProvince {
            army_id: army,
            province: 10
        }
    );
}

#[test]
fn a_nation_that_leaves_the_game_takes_its_armies_with_it() {
    let mut simulation = simulation();
    simulation
        .queue_nation_decision(NationDecision::new(SECOND, vec![recruit(C, "levy", 1)]))
        .unwrap();
    simulation.step().unwrap();
    simulation.step().unwrap();
    assert_eq!(simulation.world().armies().of_nation(SECOND).count(), 1);
    simulation.transfer_settlement(C, FIRST).unwrap();
    assert_eq!(simulation.world().armies().iter().count(), 0);
}

// ---------------------------------------------------------------- serialization

#[test]
fn military_actions_observations_and_reports_serialize_in_snake_case() {
    let mut simulation = simulation();
    simulation
        .queue_nation_decision(
            NationDecision::from_json(
                r#"{"nation_id": 0, "actions": [
                    {"type": "recruit_units", "settlement_id": 1, "unit": "levy", "units": 1}
                ]}"#,
            )
            .unwrap(),
        )
        .unwrap();
    let report = simulation.step().unwrap();
    let json = serde_json::to_value(&report.nation_decisions[0].results[0]).unwrap();
    assert_eq!(
        json["outcome"]["effect"],
        serde_json::json!({"effect": "units_recruited", "settlement_id": 1, "unit": "levy",
            "units": 1, "soldiers": 50, "ready_in_turns": 2, "army_id": null})
    );
    let report = simulation.step().unwrap();
    let military = serde_json::to_value(&report.nations[&FIRST].military).unwrap();
    assert_eq!(military["recruits_joined"][0]["units"], 1);
    let observation = observe(&simulation);
    assert_eq!(
        serde_json::from_str::<NationObservation>(&observation.to_json()).unwrap(),
        observation
    );
    let army = &serde_json::to_value(&observation).unwrap()["military"]["armies"][0];
    assert_eq!(
        army["units"][0],
        serde_json::json!({"unit_type": "levy", "soldiers": 50, "morale": 100, "raised_in": 1})
    );
}
