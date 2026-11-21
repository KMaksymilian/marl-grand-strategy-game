//! Diplomacy (gameplay specification §8): the relation matrix, unilateral
//! actions, proposals answered in the next turn or expiring, the warning
//! period, reputation, wars and peace of whole alliances with their
//! exceptions, a 10-turn truce, ownership at peace, the trade block of wars
//! and embargoes, and what nations observe.
mod common;
use common::material;
use simulation_core::ids::{NationId, SettlementId};
use simulation_core::nation::diplomacy::{
    DiplomacyError, DiplomaticEvent, DiplomaticOutcome, Relation, Treaty,
};
use simulation_core::settlement::Settlement;
use simulation_core::settlement::decision::{OfferSide, OfferSpec};
use simulation_core::simulation::{
    ActionError, ActionOutcome, NationAction, NationActionEffect, NationActionError,
    NationActionOutcome, NationDecision, NationObservation, SettlementAction, SettlementDecision,
    Simulation, SimulationTurnReport,
};
use simulation_core::trade::caravan::CaravanOutcome;
use simulation_core::world::{Map, Position, Province, World, terrain::area::Area};
use std::sync::Arc;

const A: NationId = NationId(0);
const B: NationId = NationId(1);
const C: NationId = NationId(2);
const D: NationId = NationId(3);

/// Four provinces of ten tiles in a row; settlement `n + 1` founds nation `n`
/// with its capital at the province's first tile. Settlement 1 holds 50 Wood.
fn simulation() -> Simulation {
    let territory: Vec<usize> = (0..40).map(|x| 10 * (x / 10 + 1)).collect();
    let mut world = World::new(
        Map::new(40, 1, vec![Area::new(0.5, 0.5); 40], territory),
        [10, 20, 30, 40].into_iter().map(Province::new).collect(),
    );
    for n in 0..4u32 {
        let mut settlement = Settlement::new(SettlementId(n + 1), 1_000, 5, Arc::default());
        if n == 0 {
            settlement.deposit(material("wood"), 50);
        }
        world
            .add_settlement_at(
                10 * (n as usize + 1),
                Position {
                    x: 10 * n as i32,
                    y: 0,
                },
                settlement,
            )
            .unwrap();
    }
    Simulation::new(world).unwrap()
}

fn settlement_of(nation: NationId) -> SettlementId {
    SettlementId(nation.0 + 1)
}

fn queue(simulation: &mut Simulation, nation: NationId, actions: Vec<NationAction>) {
    simulation
        .queue_nation_decision(NationDecision::new(nation, actions))
        .unwrap();
}

/// Queues the actions and runs one turn.
fn act(
    simulation: &mut Simulation,
    nation: NationId,
    actions: Vec<NationAction>,
) -> SimulationTurnReport {
    queue(simulation, nation, actions);
    simulation.step().unwrap()
}

/// Outcomes of the nation's last decision in the report.
fn outcomes(report: &SimulationTurnReport, nation: NationId) -> Vec<NationActionOutcome> {
    report
        .nation_decisions
        .iter()
        .rfind(|decision| decision.nation_id == nation)
        .map(|decision| {
            decision
                .results
                .iter()
                .map(|result| result.outcome.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn applied(outcome: &NationActionOutcome) -> DiplomaticOutcome {
    match outcome {
        NationActionOutcome::Applied {
            effect: NationActionEffect::Diplomacy { outcome },
        } => outcome.clone(),
        other => panic!("not an applied diplomatic action: {other:?}"),
    }
}

fn refused(outcome: &NationActionOutcome) -> DiplomacyError {
    match outcome {
        NationActionOutcome::Rejected {
            error: NationActionError::Diplomacy { error },
        } => error.clone(),
        other => panic!("not a diplomatic rejection: {other:?}"),
    }
}

fn relation(simulation: &Simulation, a: NationId, b: NationId) -> Relation {
    simulation.world().relation(a, b).unwrap().unwrap()
}

fn war(target: NationId) -> NationAction {
    NationAction::DeclareWar { target }
}

fn propose(target: NationId, treaty: Treaty) -> NationAction {
    NationAction::ProposeTreaty { target, treaty }
}

fn answer(proposer: NationId, treaty: Treaty, accept: bool) -> NationAction {
    NationAction::RespondToProposal {
        proposer,
        treaty,
        accept,
    }
}

/// Makes a treaty between `a` and `b` over two turns.
fn sign(simulation: &mut Simulation, a: NationId, b: NationId, treaty: Treaty) {
    act(simulation, a, vec![propose(b, treaty)]);
    let report = act(simulation, b, vec![answer(a, treaty, true)]);
    assert!(
        matches!(
            applied(&outcomes(&report, b)[0]),
            DiplomaticOutcome::ProposalAccepted { .. }
        ),
        "{treaty:?}"
    );
}

fn events(report: &SimulationTurnReport, nation: NationId) -> &[DiplomaticEvent] {
    &report.nations[&nation].diplomacy
}

// ---------------------------------------------------------------- relation matrix

#[test]
fn every_pair_starts_neutral_and_relations_are_symmetric() {
    let simulation = simulation();
    for (a, b) in [(A, B), (B, A), (C, D)] {
        assert_eq!(relation(&simulation, a, b), Relation::Neutral);
    }
    assert_eq!(simulation.world().relation(A, A), Ok(None));
    assert_eq!(
        simulation.world().relation(A, NationId(9)),
        Err(DiplomacyError::UnknownNation {
            nation_id: NationId(9)
        })
    );
}

#[test]
fn neutral_embargo_and_war_follow_the_transition_table() {
    let mut simulation = simulation();
    // Neutral -> embargo; only the imposer lifts it.
    let report = act(
        &mut simulation,
        A,
        vec![NationAction::DeclareEmbargo { target: B }],
    );
    assert_eq!(
        applied(&outcomes(&report, A)[0]),
        DiplomaticOutcome::EmbargoDeclared { target: B }
    );
    assert_eq!(
        relation(&simulation, B, A),
        Relation::Embargo { imposed_by: A }
    );
    assert_eq!(
        events(&report, B),
        &[DiplomaticEvent::EmbargoDeclared { by: A }]
    );
    let report = act(
        &mut simulation,
        B,
        vec![NationAction::LiftEmbargo { target: A }],
    );
    assert_eq!(
        refused(&outcomes(&report, B)[0]),
        DiplomacyError::NotEmbargoImposer { nation_id: A }
    );
    // Embargo -> war is allowed.
    let report = act(
        &mut simulation,
        A,
        vec![war(B), NationAction::DeclareEmbargo { target: C }],
    );
    assert!(matches!(
        applied(&outcomes(&report, A)[0]),
        DiplomaticOutcome::WarDeclared { target: B, .. }
    ));
    assert_eq!(relation(&simulation, A, B), Relation::War { aggressor: A });
    assert_eq!(
        events(&report, B),
        &[DiplomaticEvent::WarDeclared { aggressor: A }]
    );
    // War -> embargo is not a transition.
    let report = act(
        &mut simulation,
        A,
        vec![NationAction::DeclareEmbargo { target: B }],
    );
    assert_eq!(
        refused(&outcomes(&report, A)[0]),
        DiplomacyError::InvalidRelation {
            nation_id: B,
            relation: Relation::War { aggressor: A }
        }
    );
    let report = act(
        &mut simulation,
        A,
        vec![NationAction::LiftEmbargo { target: C }],
    );
    assert_eq!(
        applied(&outcomes(&report, A)[0]),
        DiplomaticOutcome::EmbargoLifted { target: C }
    );
    assert_eq!(relation(&simulation, A, C), Relation::Neutral);
}

#[test]
fn treaties_must_be_broken_before_war_and_only_from_their_own_state() {
    let mut simulation = simulation();
    sign(&mut simulation, A, B, Treaty::TradeAgreement);
    sign(&mut simulation, C, D, Treaty::Alliance);
    let report = act(
        &mut simulation,
        A,
        vec![
            war(B),
            NationAction::DeclareEmbargo { target: C },
            NationAction::BreakAlliance { target: D },
        ],
    );
    let results = outcomes(&report, A);
    assert_eq!(
        refused(&results[0]),
        DiplomacyError::InvalidRelation {
            nation_id: B,
            relation: Relation::TradeAgreement
        }
    );
    assert!(matches!(
        applied(&results[1]),
        DiplomaticOutcome::EmbargoDeclared { .. }
    ));
    assert_eq!(
        refused(&results[2]),
        DiplomacyError::InvalidRelation {
            nation_id: D,
            relation: Relation::Neutral
        }
    );
    let report = act(&mut simulation, C, vec![war(D)]);
    assert_eq!(
        refused(&outcomes(&report, C)[0]),
        DiplomacyError::InvalidRelation {
            nation_id: D,
            relation: Relation::Alliance
        }
    );
}

#[test]
fn an_alliance_follows_neutrality_or_a_trade_agreement_and_peace_only_war() {
    let mut simulation = simulation();
    sign(&mut simulation, A, B, Treaty::TradeAgreement);
    sign(&mut simulation, A, B, Treaty::Alliance);
    assert_eq!(relation(&simulation, A, B), Relation::Alliance);
    let report = act(
        &mut simulation,
        C,
        vec![
            propose(A, Treaty::Peace),
            propose(D, Treaty::TradeAgreement),
        ],
    );
    let results = outcomes(&report, C);
    assert_eq!(
        refused(&results[0]),
        DiplomacyError::InvalidRelation {
            nation_id: A,
            relation: Relation::Neutral
        }
    );
    assert!(matches!(
        applied(&results[1]),
        DiplomaticOutcome::ProposalSent { .. }
    ));
}

// ---------------------------------------------------------------- proposals

#[test]
fn a_proposal_is_answered_in_the_next_turn_or_expires() {
    let mut simulation = simulation();
    // Turn 1: A proposes; B (deciding later in the same turn) cannot answer yet.
    queue(&mut simulation, A, vec![propose(B, Treaty::TradeAgreement)]);
    queue(
        &mut simulation,
        B,
        vec![answer(A, Treaty::TradeAgreement, true)],
    );
    let report = simulation.step().unwrap();
    assert_eq!(
        refused(&outcomes(&report, B)[0]),
        DiplomacyError::NoSuchProposal {
            nation_id: A,
            treaty: Treaty::TradeAgreement
        }
    );
    assert_eq!(
        events(&report, B),
        &[DiplomaticEvent::ProposalReceived {
            from: A,
            treaty: Treaty::TradeAgreement
        }]
    );
    let incoming = simulation
        .observe_nation(B)
        .unwrap()
        .diplomacy
        .incoming_proposals;
    assert_eq!(incoming.len(), 1);
    assert!(incoming[0].answerable);
    // Turn 2: no answer; the proposal expires at the end of the turn.
    let report = simulation.step().unwrap();
    let expired = DiplomaticEvent::ProposalExpired {
        from: A,
        to: B,
        treaty: Treaty::TradeAgreement,
    };
    assert_eq!(events(&report, A), std::slice::from_ref(&expired));
    assert_eq!(events(&report, B), &[expired]);
    // Turn 3: too late.
    let report = act(
        &mut simulation,
        B,
        vec![answer(A, Treaty::TradeAgreement, true)],
    );
    assert!(matches!(
        refused(&outcomes(&report, B)[0]),
        DiplomacyError::NoSuchProposal { .. }
    ));
    assert_eq!(relation(&simulation, A, B), Relation::Neutral);
}

#[test]
fn accepted_and_rejected_proposals_are_reported_to_the_proposer() {
    let mut simulation = simulation();
    act(
        &mut simulation,
        A,
        vec![
            propose(B, Treaty::TradeAgreement),
            propose(C, Treaty::Alliance),
        ],
    );
    queue(
        &mut simulation,
        B,
        vec![answer(A, Treaty::TradeAgreement, true)],
    );
    queue(&mut simulation, C, vec![answer(A, Treaty::Alliance, false)]);
    let report = simulation.step().unwrap();
    assert_eq!(
        applied(&outcomes(&report, B)[0]),
        DiplomaticOutcome::ProposalAccepted {
            proposer: A,
            treaty: Treaty::TradeAgreement,
            relation: Relation::TradeAgreement,
            allied_truces: vec![]
        }
    );
    assert_eq!(
        applied(&outcomes(&report, C)[0]),
        DiplomaticOutcome::ProposalRejected {
            proposer: A,
            treaty: Treaty::Alliance
        }
    );
    assert_eq!(
        events(&report, A),
        &[
            DiplomaticEvent::ProposalAccepted {
                by: B,
                treaty: Treaty::TradeAgreement
            },
            DiplomaticEvent::ProposalRejected {
                by: C,
                treaty: Treaty::Alliance
            }
        ]
    );
    assert_eq!(relation(&simulation, A, C), Relation::Neutral);
    assert!(simulation.world().diplomacy().proposals().is_empty());
}

#[test]
fn proposals_are_unique_and_one_action_per_nation_per_decision() {
    let mut simulation = simulation();
    act(&mut simulation, A, vec![propose(B, Treaty::Alliance)]);
    let report = act(
        &mut simulation,
        A,
        vec![
            propose(B, Treaty::Alliance),
            propose(C, Treaty::Alliance),
            NationAction::DeclareEmbargo { target: C },
            propose(A, Treaty::Alliance),
            war(NationId(9)),
        ],
    );
    let results = outcomes(&report, A);
    assert_eq!(
        refused(&results[0]),
        DiplomacyError::ProposalAlreadyPending {
            nation_id: B,
            treaty: Treaty::Alliance
        }
    );
    assert!(matches!(
        applied(&results[1]),
        DiplomaticOutcome::ProposalSent { .. }
    ));
    assert_eq!(
        results[2],
        NationActionOutcome::Rejected {
            error: NationActionError::Conflict { with_index: 1 }
        }
    );
    assert_eq!(refused(&results[3]), DiplomacyError::DiplomacyWithSelf);
    assert_eq!(
        refused(&results[4]),
        DiplomacyError::UnknownNation {
            nation_id: NationId(9)
        }
    );
}

// ---------------------------------------------------------------- warning and reputation

#[test]
fn breaking_a_treaty_gives_a_warning_turn_and_costs_reputation() {
    let mut simulation = simulation();
    sign(&mut simulation, A, B, Treaty::Alliance);
    // Two decisions of A in the same turn: break, then war. War must wait.
    queue(
        &mut simulation,
        A,
        vec![NationAction::BreakAlliance { target: B }],
    );
    queue(&mut simulation, A, vec![war(B)]);
    let report = simulation.step().unwrap();
    let decisions: Vec<_> = report
        .nation_decisions
        .iter()
        .filter(|decision| decision.nation_id == A)
        .collect();
    assert!(decisions[0].results[0].is_applied());
    assert_eq!(
        refused(&decisions[1].results[0].outcome),
        DiplomacyError::WarningPeriod { nation_id: B }
    );
    assert_eq!(
        events(&report, B),
        &[DiplomaticEvent::TreatyBroken {
            by: A,
            treaty: Treaty::Alliance
        }]
    );
    assert_eq!(simulation.world().diplomacy().reputation(A), 1);
    assert_eq!(simulation.world().diplomacy().reputation(B), 0);
    // Next turn: war is possible, and so soon after the break it counts again.
    let report = act(&mut simulation, A, vec![war(B)]);
    assert!(matches!(
        applied(&outcomes(&report, A)[0]),
        DiplomaticOutcome::WarDeclared { target: B, .. }
    ));
    assert_eq!(simulation.world().diplomacy().reputation(A), 2);
}

#[test]
fn a_war_long_after_a_break_does_not_count_again() {
    let mut simulation = simulation();
    sign(&mut simulation, A, B, Treaty::TradeAgreement);
    act(
        &mut simulation,
        A,
        vec![NationAction::BreakTradeAgreement { target: B }],
    );
    for _ in 0..5 {
        simulation.step().unwrap();
    }
    act(&mut simulation, A, vec![war(B)]);
    assert_eq!(relation(&simulation, A, B), Relation::War { aggressor: A });
    assert_eq!(simulation.world().diplomacy().reputation(A), 1);
}

// ---------------------------------------------------------------- alliances and peace

#[test]
fn both_whole_alliances_go_to_war() {
    let mut simulation = simulation();
    // A's alliance: A and C. B's alliance: B and D.
    sign(&mut simulation, A, C, Treaty::Alliance);
    sign(&mut simulation, B, D, Treaty::Alliance);
    let report = act(&mut simulation, A, vec![war(B)]);
    assert_eq!(
        applied(&outcomes(&report, A)[0]),
        DiplomaticOutcome::WarDeclared {
            target: B,
            attackers_joined: vec![C],
            defenders_joined: vec![D]
        }
    );
    // Every member of one alliance fights every member of the other; the
    // member of the attacking alliance is the aggressor of its pair.
    assert_eq!(relation(&simulation, A, B), Relation::War { aggressor: A });
    assert_eq!(relation(&simulation, A, D), Relation::War { aggressor: A });
    assert_eq!(relation(&simulation, C, B), Relation::War { aggressor: C });
    assert_eq!(relation(&simulation, C, D), Relation::War { aggressor: C });
    assert_eq!(relation(&simulation, A, C), Relation::Alliance);
    assert_eq!(relation(&simulation, B, D), Relation::Alliance);
    let joined = |enemy| DiplomaticEvent::WarJoined {
        enemy,
        aggressor: A,
        target: B,
    };
    assert_eq!(events(&report, A), &[joined(D)]);
    assert_eq!(
        events(&report, B),
        &[DiplomaticEvent::WarDeclared { aggressor: A }, joined(C)]
    );
    assert_eq!(events(&report, C), &[joined(B), joined(D)]);
    assert_eq!(events(&report, D), &[joined(A), joined(C)]);
    // Joining an alliance's war breaks no treaty.
    for nation in [B, C, D] {
        assert_eq!(simulation.world().diplomacy().reputation(nation), 0);
    }
}

#[test]
fn nations_allied_with_both_sides_stay_out_and_allies_never_fight() {
    let mut simulation = simulation();
    // C is allied with A and with B; D is allied with B and with C.
    sign(&mut simulation, A, C, Treaty::Alliance);
    sign(&mut simulation, B, C, Treaty::Alliance);
    sign(&mut simulation, B, D, Treaty::Alliance);
    sign(&mut simulation, C, D, Treaty::Alliance);
    let report = act(&mut simulation, A, vec![war(B)]);
    assert_eq!(
        applied(&outcomes(&report, A)[0]),
        DiplomaticOutcome::WarDeclared {
            target: B,
            attackers_joined: vec![],
            defenders_joined: vec![D]
        }
    );
    // C stays out: allied with both sides, it keeps both alliances.
    assert_eq!(relation(&simulation, A, C), Relation::Alliance);
    assert_eq!(relation(&simulation, B, C), Relation::Alliance);
    // D defends B against A and keeps its alliance with C.
    assert_eq!(relation(&simulation, A, D), Relation::War { aggressor: A });
    assert_eq!(relation(&simulation, C, D), Relation::Alliance);
}

#[test]
fn an_ally_in_a_truce_with_the_other_side_joins_the_war() {
    let mut simulation = simulation();
    act(&mut simulation, A, vec![war(C)]);
    act(&mut simulation, A, vec![propose(C, Treaty::Peace)]);
    act(&mut simulation, C, vec![answer(A, Treaty::Peace, true)]);
    assert!(matches!(
        relation(&simulation, A, C),
        Relation::Truce { .. }
    ));
    sign(&mut simulation, B, C, Treaty::Alliance);
    // A may not attack C during the truce, but attacking C's ally B brings
    // C into the war: the alliance comes first.
    let report = act(&mut simulation, A, vec![war(B)]);
    assert_eq!(
        applied(&outcomes(&report, A)[0]),
        DiplomaticOutcome::WarDeclared {
            target: B,
            attackers_joined: vec![],
            defenders_joined: vec![C]
        }
    );
    assert_eq!(relation(&simulation, A, C), Relation::War { aggressor: A });
}

#[test]
fn peace_ends_the_war_of_both_alliances_keeps_conquests_and_starts_a_truce() {
    let mut simulation = simulation();
    // B also owns settlement 4 (D leaves the game), and C is B's ally.
    simulation.transfer_settlement(settlement_of(D), B).unwrap();
    sign(&mut simulation, B, C, Treaty::Alliance);
    let report = act(&mut simulation, A, vec![war(B)]);
    assert_eq!(
        applied(&outcomes(&report, A)[0]),
        DiplomaticOutcome::WarDeclared {
            target: B,
            attackers_joined: vec![],
            defenders_joined: vec![C]
        }
    );
    // A conquers settlement 4 during the war (the war system will do this).
    simulation.transfer_settlement(settlement_of(D), A).unwrap();
    act(&mut simulation, B, vec![propose(A, Treaty::Peace)]);
    let report = act(&mut simulation, A, vec![answer(B, Treaty::Peace, true)]);
    assert_eq!(
        applied(&outcomes(&report, A)[0]),
        DiplomaticOutcome::ProposalAccepted {
            proposer: B,
            treaty: Treaty::Peace,
            relation: Relation::Truce { turns_left: 10 },
            allied_truces: vec![[A, C]]
        }
    );
    // Peace of B ends the war of B's whole alliance: A and C are in a truce
    // too. Both count down from the end of this turn.
    assert_eq!(
        relation(&simulation, A, B),
        Relation::Truce { turns_left: 9 }
    );
    assert_eq!(
        relation(&simulation, A, C),
        Relation::Truce { turns_left: 9 }
    );
    assert_eq!(
        events(&report, C),
        &[DiplomaticEvent::PeaceJoined {
            enemy: A,
            proposer: B,
            accepter: A
        }]
    );
    // Status quo: the conquered settlement stays with A.
    assert_eq!(
        simulation.world().settlement_owner(settlement_of(D)),
        Some(A)
    );
    assert_eq!(
        simulation.world().settlement_owner(settlement_of(B)),
        Some(B)
    );
    // No war during the truce; trade is allowed as in neutrality.
    let report = act(&mut simulation, A, vec![war(B)]);
    assert_eq!(
        refused(&outcomes(&report, A)[0]),
        DiplomacyError::TruceInForce {
            nation_id: B,
            turns_left: 9
        }
    );
    for other in [B, C] {
        assert!(
            simulation
                .world()
                .trade_allowed(settlement_of(A), settlement_of(other))
        );
    }
    let mut last = report;
    for _ in 0..8 {
        last = simulation.step().unwrap();
    }
    assert_eq!(relation(&simulation, A, B), Relation::Neutral);
    assert_eq!(relation(&simulation, A, C), Relation::Neutral);
    assert!(events(&last, A).contains(&DiplomaticEvent::TruceEnded { with: B }));
    assert!(events(&last, C).contains(&DiplomaticEvent::TruceEnded { with: A }));
    let report = act(&mut simulation, A, vec![war(B)]);
    assert!(matches!(
        applied(&outcomes(&report, A)[0]),
        DiplomaticOutcome::WarDeclared { .. }
    ));
}

#[test]
fn a_nation_that_leaves_the_game_leaves_no_relations_or_proposals() {
    let mut simulation = simulation();
    act(
        &mut simulation,
        A,
        vec![war(B), propose(C, Treaty::Alliance)],
    );
    act(&mut simulation, B, vec![propose(D, Treaty::TradeAgreement)]);
    simulation.transfer_settlement(settlement_of(B), A).unwrap();
    let diplomacy = &simulation.observe_nation(A).unwrap().diplomacy;
    assert!(!diplomacy.relations.contains_key(&B));
    assert!(!diplomacy.reputation.contains_key(&B));
    assert!(
        simulation
            .world()
            .diplomacy()
            .proposals()
            .iter()
            .all(|proposal| proposal.from != B && proposal.to != B)
    );
}

// ---------------------------------------------------------------- trade

fn caravan(receiver: SettlementId) -> SettlementDecision {
    SettlementDecision::new(
        SettlementId(1),
        vec![SettlementAction::OrderCaravan {
            receiver_id: receiver,
            offers: vec![OfferSpec {
                resource: material("wood"),
                side: OfferSide::Sell,
                units: 5,
                price_per_unit: 1,
            }],
        }],
    )
}

#[test]
fn wars_and_embargoes_block_new_caravans_and_neutral_partners_do_not() {
    let mut simulation = simulation();
    queue(
        &mut simulation,
        A,
        vec![war(B), NationAction::DeclareEmbargo { target: C }],
    );
    for receiver in [settlement_of(B), settlement_of(C), settlement_of(D)] {
        simulation.queue_decision(caravan(receiver)).unwrap();
    }
    let report = simulation.step().unwrap();
    let results: Vec<_> = report
        .decisions
        .iter()
        .map(|decision| decision.results[0].outcome.clone())
        .collect();
    for (outcome, receiver) in results[..2]
        .iter()
        .zip([settlement_of(B), settlement_of(C)])
    {
        assert_eq!(
            outcome,
            &ActionOutcome::Rejected {
                error: ActionError::TradeBlocked {
                    receiver_id: receiver
                }
            }
        );
    }
    assert!(matches!(results[2], ActionOutcome::Applied { .. }));
    let observation = simulation.observe(SettlementId(1)).unwrap();
    let partners: Vec<_> = observation
        .trade_partners
        .iter()
        .map(|partner| (partner.nation_id, partner.trade_allowed))
        .collect();
    assert_eq!(partners, vec![(B, false), (C, false), (D, true)]);
    assert_eq!(observation.nation.at_war_with, vec![B]);
}

#[test]
fn a_caravan_on_the_road_when_war_breaks_out_turns_back_without_trading() {
    let mut simulation = simulation();
    // Settlement 1 sends 5 Wood to settlement 4, 30 steps away (3 turns).
    simulation
        .queue_decision(caravan(settlement_of(D)))
        .unwrap();
    simulation.step().unwrap();
    act(&mut simulation, D, vec![war(A)]);
    let mut returned = Vec::new();
    for _ in 0..10 {
        let report = simulation.step().unwrap();
        returned.extend(
            report.settlements[&SettlementId(1)]
                .trade
                .caravans_returned
                .iter()
                .map(|caravan| caravan.outcome),
        );
    }
    assert_eq!(returned, vec![CaravanOutcome::TradeBlocked]);
    assert_eq!(
        simulation
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(material("wood")),
        50,
        "the goods came back"
    );
}

// ---------------------------------------------------------------- observation

#[test]
fn nations_observe_relations_their_own_proposals_and_everyones_reputation() {
    let mut simulation = simulation();
    sign(&mut simulation, A, B, Treaty::TradeAgreement);
    queue(
        &mut simulation,
        A,
        vec![NationAction::BreakTradeAgreement { target: B }],
    );
    queue(&mut simulation, C, vec![propose(D, Treaty::Alliance)]);
    simulation.step().unwrap();
    act(&mut simulation, A, vec![propose(C, Treaty::Alliance)]);
    let observation = simulation.observe_nation(C).unwrap();
    let diplomacy = &observation.diplomacy;
    assert_eq!(diplomacy.relations.len(), 3);
    assert!(
        diplomacy
            .relations
            .values()
            .all(|relation| *relation == Relation::Neutral)
    );
    assert_eq!(diplomacy.incoming_proposals.len(), 1);
    assert_eq!(diplomacy.incoming_proposals[0].from, A);
    // C's own proposal to D expired; others' negotiations are not shown to C.
    assert!(diplomacy.outgoing_proposals.is_empty());
    assert_eq!(diplomacy.reputation[&A], 1);
    assert_eq!(diplomacy.reputation[&C], 0);
    assert_eq!(
        serde_json::from_str::<NationObservation>(&observation.to_json()).unwrap(),
        observation
    );
}

#[test]
fn diplomatic_actions_and_results_serialize_in_snake_case() {
    let mut simulation = simulation();
    simulation
        .queue_nation_decision(
            NationDecision::from_json(
                r#"{"nation_id": 0, "actions": [
                    {"type": "declare_war", "target": 1},
                    {"type": "propose_treaty", "target": 1, "treaty": "peace"}
                ]}"#,
            )
            .unwrap(),
        )
        .unwrap();
    let report = simulation.step().unwrap();
    let json = serde_json::to_value(&report.nation_decisions[0].results).unwrap();
    assert_eq!(
        json[0]["outcome"]["effect"],
        serde_json::json!({"effect": "diplomacy", "outcome":
            {"effect": "war_declared", "target": 1,
             "attackers_joined": [], "defenders_joined": []}})
    );
    assert_eq!(
        json[1]["outcome"]["error"],
        serde_json::json!({"reason": "conflict", "with_index": 0})
    );
    let nation = serde_json::to_value(&report.nations[&B]).unwrap();
    assert_eq!(
        nation["diplomacy"],
        serde_json::json!([{"event": "war_declared", "aggressor": 0}])
    );
}
