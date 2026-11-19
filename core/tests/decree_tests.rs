//! Decrees (gameplay specification §7): the upkeep paid every active turn,
//! automatic revocation when unpaid, delayed start,
//! minimum duration, cooldown, slots, effects as data for the systems that
//! apply them, and the one-shot capital relocation with its refund.
use simulation_core::ids::{NationId, SettlementId};
use simulation_core::nation::decree::{
    CapitalRelocation, Decree, DecreeEffects, DecreeState, ProductionScope, RelocationFailure,
    RelocationOutcome,
};
use simulation_core::settlement::Settlement;
use simulation_core::simulation::{
    NationAction, NationActionEffect, NationActionError, NationActionOutcome, NationDecision,
    NationObservation, Simulation, SimulationTurnReport,
};
use simulation_core::world::{Map, Province, World, terrain::area::Area};
use std::collections::BTreeMap;
use std::sync::Arc;

const A: SettlementId = SettlementId(1);
const B: SettlementId = SettlementId(2);
const C: SettlementId = SettlementId(3);
const FIRST: NationId = NationId(0);
const SECOND: NationId = NationId(1);

/// Provinces 10, 20, 30, 40 in a row. A founds nation 0 and B joins it; C
/// founds nation 1; province 40 is empty. Nation 0 holds `treasury` gold.
fn simulation(treasury: u32) -> Simulation {
    let mut world = World::new(
        Map::new(4, 1, vec![Area::new(0.5, 0.5); 4], vec![10, 20, 30, 40]),
        [10, 20, 30, 40].into_iter().map(Province::new).collect(),
    );
    world
        .add_settlement(10, Settlement::new(A, 100, 5, Arc::default()))
        .unwrap();
    world
        .add_settlement_to_nation(FIRST, 20, Settlement::new(B, 100, 9, Arc::default()))
        .unwrap();
    world
        .add_settlement(30, Settlement::new(C, 100, 3, Arc::default()))
        .unwrap();
    world.deposit_nation_gold(FIRST, treasury).unwrap();
    Simulation::new(world).unwrap()
}

fn queue(simulation: &mut Simulation, actions: Vec<NationAction>) {
    simulation
        .queue_nation_decision(NationDecision::new(FIRST, actions))
        .unwrap();
}

/// Queues nation 0's actions and runs one turn.
fn act(simulation: &mut Simulation, actions: Vec<NationAction>) -> SimulationTurnReport {
    queue(simulation, actions);
    simulation.step().unwrap()
}

fn enact(decree: Decree) -> NationAction {
    NationAction::EnactDecree { decree }
}

fn revoke(decree: Decree) -> NationAction {
    NationAction::RevokeDecree { decree }
}

fn relocate(settlement_id: SettlementId) -> NationAction {
    NationAction::RelocateCapital { settlement_id }
}

/// Outcomes of nation 0's last decision in the report.
fn outcomes(report: &SimulationTurnReport) -> Vec<NationActionOutcome> {
    report
        .nation_decisions
        .iter()
        .rfind(|decision| decision.nation_id == FIRST)
        .map(|decision| {
            decision
                .results
                .iter()
                .map(|result| result.outcome.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn error(outcome: &NationActionOutcome) -> NationActionError {
    match outcome {
        NationActionOutcome::Rejected { error } => error.clone(),
        NationActionOutcome::Applied { effect } => panic!("applied: {effect:?}"),
    }
}

fn applied(outcome: &NationActionOutcome) -> bool {
    matches!(outcome, NationActionOutcome::Applied { .. })
}

fn treasury(simulation: &Simulation) -> u32 {
    simulation.world().nation(FIRST).unwrap().treasury()
}

fn state(simulation: &Simulation, decree: Decree) -> Option<DecreeState> {
    simulation
        .world()
        .nation(FIRST)
        .unwrap()
        .decrees()
        .state(decree)
}

// ---------------------------------------------------------------- upkeep and timing

#[test]
fn a_decree_works_from_the_next_turn_and_costs_its_upkeep_every_active_turn() {
    let mut simulation = simulation(1_000);
    queue(&mut simulation, vec![enact(Decree::AgriculturalDecree)]);
    // Before the turn: nothing enacted, nothing in effect.
    assert_eq!(
        simulation.world().decree_effects(A),
        DecreeEffects::default()
    );
    let report = simulation.step().unwrap();
    assert_eq!(
        outcomes(&report)[0],
        NationActionOutcome::Applied {
            // 10 gold × 2 settlements × 1 per active turn.
            effect: NationActionEffect::DecreeEnacted {
                decree: Decree::AgriculturalDecree,
                upkeep: 20
            }
        }
    );
    // Turn 1: pending, nothing paid. The end-of-turn state is active.
    assert_eq!(treasury(&simulation), 1_000);
    assert!(report.nations[&FIRST].decree_upkeep.paid.is_empty());
    assert_eq!(
        report.nations[&FIRST].decrees,
        BTreeMap::from([(
            Decree::AgriculturalDecree,
            DecreeState::Active { turns_active: 0 }
        )])
    );
    let effects = simulation.world().decree_effects(B);
    assert_eq!(effects.production[&ProductionScope::FoodTier1], 0.2);
    assert_eq!(
        effects.production[&ProductionScope::BuildingMaterialTier1],
        -0.1
    );
    // Turns 2..=6: 20 gold every turn.
    for turn in 1..=5 {
        let report = simulation.step().unwrap();
        let upkeep = &report.nations[&FIRST].decree_upkeep;
        assert_eq!(upkeep.paid, vec![Decree::AgriculturalDecree]);
        assert_eq!(upkeep.gold_paid, 20);
        assert_eq!(treasury(&simulation), 1_000 - 20 * turn);
    }
}

#[test]
fn a_decree_the_treasury_cannot_pay_is_revoked_like_any_other() {
    // Games cost 10 × 2 × 4 = 80 per turn; the treasury holds 100.
    let mut simulation = simulation(100);
    let report = act(&mut simulation, vec![enact(Decree::Games)]);
    assert!(applied(&outcomes(&report)[0]), "enacting needs no gold");
    let report = simulation.step().unwrap();
    assert_eq!(report.nations[&FIRST].decree_upkeep.gold_paid, 80);
    assert_eq!(treasury(&simulation), 20);
    let report = simulation.step().unwrap();
    let upkeep = &report.nations[&FIRST].decree_upkeep;
    assert_eq!(upkeep.revoked_unpaid, vec![Decree::Games]);
    assert!(upkeep.paid.is_empty());
    assert_eq!(treasury(&simulation), 20, "an unpaid decree takes nothing");
    assert_eq!(
        state(&simulation, Decree::Games),
        Some(DecreeState::Cooldown { turns_left: 4 })
    );
    assert_eq!(
        simulation.world().decree_effects(A),
        DecreeEffects::default()
    );
}

#[test]
fn upkeep_scales_with_the_settlements_owned_at_payment() {
    let mut simulation = simulation(1_000);
    act(&mut simulation, vec![enact(Decree::GreatConstruction)]);
    let report = simulation.step().unwrap();
    // 10 × 2 settlements × 2.
    assert_eq!(report.nations[&FIRST].decree_upkeep.gold_paid, 40);
    simulation.transfer_settlement(C, FIRST).unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(report.nations[&FIRST].decree_upkeep.gold_paid, 60);
}

#[test]
fn effects_belong_to_the_owners_settlements_only() {
    let mut simulation = simulation(1_000);
    act(&mut simulation, vec![enact(Decree::Games)]);
    assert_eq!(simulation.world().decree_effects(A).happiness, 15.0);
    assert_eq!(simulation.world().decree_effects(B).happiness, 15.0);
    assert_eq!(
        simulation.world().decree_effects(C),
        DecreeEffects::default()
    );
    assert_eq!(
        simulation.world().decree_effects(SettlementId(9)),
        DecreeEffects::default()
    );
}

// ---------------------------------------------------------------- duration and cooldown

#[test]
fn a_decree_can_be_revoked_only_after_five_active_turns() {
    let mut simulation = simulation(1_000);
    act(&mut simulation, vec![enact(Decree::Games)]);
    for turns_left in (1..=5).rev() {
        let report = act(&mut simulation, vec![revoke(Decree::Games)]);
        assert_eq!(
            error(&outcomes(&report)[0]),
            NationActionError::DecreeMinimumDuration {
                decree: Decree::Games,
                turns_left
            }
        );
    }
    let report = act(&mut simulation, vec![revoke(Decree::Games)]);
    assert_eq!(
        outcomes(&report)[0],
        NationActionOutcome::Applied {
            effect: NationActionEffect::DecreeRevoked {
                decree: Decree::Games,
                cooldown_turns: 5
            }
        }
    );
    assert_eq!(
        simulation.world().decree_effects(A),
        DecreeEffects::default()
    );
    // Paid for active turns 2..=6; the turn of the revocation costs nothing.
    assert_eq!(treasury(&simulation), 1_000 - 5 * 80);
}

#[test]
fn a_revoked_decree_can_be_enacted_again_after_five_turns() {
    let mut simulation = simulation(1_000);
    act(&mut simulation, vec![enact(Decree::FreeTrade)]);
    for _ in 0..5 {
        simulation.step().unwrap();
    }
    // Revoke, and the same decree cannot come back in this decision.
    let report = act(
        &mut simulation,
        vec![revoke(Decree::FreeTrade), enact(Decree::FreeTrade)],
    );
    let results = outcomes(&report);
    assert!(applied(&results[0]));
    assert_eq!(
        error(&results[1]),
        NationActionError::Conflict { with_index: 0 }
    );
    for turns_left in (1..=4).rev() {
        let report = act(&mut simulation, vec![enact(Decree::FreeTrade)]);
        assert_eq!(
            error(&outcomes(&report)[0]),
            NationActionError::DecreeOnCooldown {
                decree: Decree::FreeTrade,
                turns_left
            }
        );
    }
    assert_eq!(state(&simulation, Decree::FreeTrade), None);
    let report = act(&mut simulation, vec![enact(Decree::FreeTrade)]);
    assert!(applied(&outcomes(&report)[0]));
    // Five paid active turns before the revocation; the new one is pending.
    assert_eq!(treasury(&simulation), 1_000 - 5 * 20);
}

// ---------------------------------------------------------------- slots

#[test]
fn pending_and_active_decrees_use_the_two_slots_and_a_revocation_frees_one() {
    let mut simulation = simulation(10_000);
    // Turn 1: two decrees fit; a third does not, and a repeat conflicts.
    queue(
        &mut simulation,
        vec![
            enact(Decree::Games),
            enact(Decree::Games),
            enact(Decree::Discipline),
            enact(Decree::Protectionism),
        ],
    );
    // A second decision of the same turn sees the pending decrees.
    queue(
        &mut simulation,
        vec![revoke(Decree::Discipline), enact(Decree::Discipline)],
    );
    let report = simulation.step().unwrap();
    let first = &report.nation_decisions[0].results;
    assert!(first[0].is_applied());
    assert_eq!(
        error(&first[1].outcome),
        NationActionError::Conflict { with_index: 0 }
    );
    assert!(first[2].is_applied());
    assert_eq!(
        error(&first[3].outcome),
        NationActionError::NoFreeDecreeSlot { slots: 2 }
    );
    let second = outcomes(&report);
    assert_eq!(
        error(&second[0]),
        NationActionError::DecreeNotActive {
            decree: Decree::Discipline
        },
        "a pending decree cannot be revoked"
    );
    assert_eq!(
        error(&second[1]),
        NationActionError::DecreeAlreadyEnacted {
            decree: Decree::Discipline
        }
    );
    for _ in 0..5 {
        simulation.step().unwrap();
    }
    // Revoking frees a slot for another decree in the same decision.
    let report = act(
        &mut simulation,
        vec![revoke(Decree::Games), enact(Decree::Protectionism)],
    );
    assert!(outcomes(&report).iter().all(applied));
    let decrees = simulation.observe_nation(FIRST).unwrap().decrees;
    assert_eq!((decrees.slots, decrees.used_slots), (2, 2));
}

// ---------------------------------------------------------------- relocation

#[test]
fn the_capital_moves_one_turn_after_the_relocation_is_paid() {
    let mut simulation = simulation(1_000);
    let report = act(&mut simulation, vec![relocate(B)]);
    assert_eq!(
        outcomes(&report)[0],
        NationActionOutcome::Applied {
            effect: NationActionEffect::CapitalRelocationOrdered {
                settlement_id: B,
                cost: 200
            }
        }
    );
    assert_eq!(treasury(&simulation), 800);
    assert_eq!(simulation.world().nation(FIRST).unwrap().capital(), A);
    assert_eq!(
        simulation
            .observe_nation(FIRST)
            .unwrap()
            .decrees
            .pending_relocation,
        Some(CapitalRelocation {
            target: B,
            paid: 200
        })
    );

    let report = simulation.step().unwrap();
    assert_eq!(
        report.nations[&FIRST].capital_relocation,
        Some(RelocationOutcome::Completed { from: A, to: B })
    );
    let nation = simulation.world().nation(FIRST).unwrap();
    assert_eq!(nation.capital(), B);
    assert_eq!(nation.capital_loss_turns(), 0, "a chosen move is no loss");
    assert_eq!(treasury(&simulation), 800);
    assert!(simulation.observe(B).unwrap().nation.is_capital);
}

#[test]
fn a_relocation_to_a_settlement_under_attack_or_lost_is_refunded() {
    let mut simulation = self::simulation(1_000);
    act(&mut simulation, vec![relocate(B)]);
    simulation.set_settlement_under_attack(B, true).unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.nations[&FIRST].capital_relocation,
        Some(RelocationOutcome::Refunded {
            target: B,
            reason: RelocationFailure::UnderAttack,
            refund: 200
        })
    );
    assert_eq!(simulation.world().nation(FIRST).unwrap().capital(), A);
    assert_eq!(treasury(&simulation), 1_000);

    let mut simulation = self::simulation(1_000);
    act(&mut simulation, vec![relocate(B)]);
    simulation.transfer_settlement(B, SECOND).unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.nations[&FIRST].capital_relocation,
        Some(RelocationOutcome::Refunded {
            target: B,
            reason: RelocationFailure::NotOwned,
            // The refund is what was paid: 100 × the two settlements of then.
            refund: 200
        })
    );
    assert_eq!(treasury(&simulation), 1_000);
}

#[test]
fn relocation_orders_are_checked_and_use_no_decree_slot() {
    let mut simulation = simulation(1_000);
    simulation.set_settlement_under_attack(B, true).unwrap();
    let report = act(
        &mut simulation,
        vec![
            relocate(A),
            relocate(C),
            relocate(SettlementId(9)),
            relocate(B),
        ],
    );
    let errors: Vec<_> = outcomes(&report).iter().map(error).collect();
    assert_eq!(
        errors,
        vec![
            NationActionError::AlreadyCapital { settlement_id: A },
            NationActionError::SettlementNotOwned { settlement_id: C },
            NationActionError::UnknownSettlement {
                settlement_id: SettlementId(9)
            },
            NationActionError::SettlementUnderAttack { settlement_id: B },
        ]
    );
    simulation.set_settlement_under_attack(B, false).unwrap();
    // Two decrees plus a relocation are all accepted; a second relocation conflicts.
    let report = act(
        &mut simulation,
        vec![
            enact(Decree::Discipline),
            enact(Decree::Protectionism),
            relocate(B),
            relocate(B),
        ],
    );
    let results = outcomes(&report);
    assert!(results[..3].iter().all(applied));
    assert_eq!(
        error(&results[3]),
        NationActionError::Conflict { with_index: 2 }
    );
    // Decrees cost nothing while pending; the relocation is paid now.
    assert_eq!(treasury(&simulation), 1_000 - 200);
}

// ---------------------------------------------------------------- observation

#[test]
fn nations_and_settlements_observe_decrees() {
    let mut simulation = simulation(1_000);
    act(
        &mut simulation,
        vec![enact(Decree::AgriculturalDecree), relocate(B)],
    );
    let observation = simulation.observe_nation(FIRST).unwrap();
    let decrees = &observation.decrees;
    assert_eq!((decrees.slots, decrees.used_slots), (2, 1));
    assert_eq!(
        decrees.states,
        BTreeMap::from([(
            Decree::AgriculturalDecree,
            DecreeState::Active { turns_active: 0 }
        )])
    );
    assert_eq!(decrees.upkeep.len(), Decree::ALL.len());
    assert_eq!(decrees.upkeep[&Decree::Games], 80);
    assert_eq!(decrees.relocation_cost, 200);
    assert_eq!(decrees.effects.production[&ProductionScope::FoodTier1], 0.2);
    assert_eq!(
        serde_json::from_str::<NationObservation>(&observation.to_json()).unwrap(),
        observation
    );
    assert_eq!(
        simulation.observe(A).unwrap().nation.active_decrees,
        vec![Decree::AgriculturalDecree]
    );
    assert!(
        simulation
            .observe(C)
            .unwrap()
            .nation
            .active_decrees
            .is_empty()
    );
}

#[test]
fn decree_actions_and_results_serialize_in_snake_case() {
    let mut simulation = simulation(1_000);
    simulation
        .queue_nation_decision(
            NationDecision::from_json(
                r#"{"nation_id": 0, "actions": [
                    {"type": "enact_decree", "decree": "great_construction"},
                    {"type": "relocate_capital", "settlement_id": 2}
                ]}"#,
            )
            .unwrap(),
        )
        .unwrap();
    let report = simulation.step().unwrap();
    let json = serde_json::to_value(&report.nation_decisions[0].results).unwrap();
    assert_eq!(
        json[0]["outcome"]["effect"],
        serde_json::json!({"effect": "decree_enacted", "decree": "great_construction", "upkeep": 40})
    );
    assert_eq!(
        json[1]["outcome"]["effect"],
        serde_json::json!({"effect": "capital_relocation_ordered", "settlement_id": 2, "cost": 200})
    );
    let report = simulation.step().unwrap();
    let nation = serde_json::to_value(&report.nations[&FIRST]).unwrap();
    assert_eq!(
        nation["capital_relocation"],
        serde_json::json!({"outcome": "completed", "from": 1, "to": 2})
    );
    assert_eq!(
        nation["decrees"],
        serde_json::json!({"great_construction": {"state": "active", "turns_active": 1}})
    );
}
