//! Nations (gameplay specification §5): every settlement belongs to a nation,
//! ownership authority, transfer, capital succession, extinction, national
//! stock capacity, subsidies, the nation decision contract, the nation's own
//! report and its observations.
use simulation_core::economy::ResourceType::{self, Grain, Labour, Stone, Wood};
use simulation_core::ids::{NationId, SettlementId};
use simulation_core::nation::Nation;
use simulation_core::settlement::Settlement;
use simulation_core::settlement::decision::{OfferSide, OfferSpec};
use simulation_core::simulation::{
    NationAction, NationActionEffect, NationActionError, NationActionKind, NationActionOutcome,
    NationDecision, NationObservation, OwnershipChange, SettlementAction, SettlementDecision,
    Simulation,
};
use simulation_core::world::{Map, Province, World, WorldError, terrain::area::Area};
use std::collections::BTreeMap;
use std::sync::Arc;

const A: SettlementId = SettlementId(1);
const B: SettlementId = SettlementId(2);
const C: SettlementId = SettlementId(3);
const D: SettlementId = SettlementId(4);
const FIRST: NationId = NationId(0);
const SECOND: NationId = NationId(1);

fn settlement(id: SettlementId, population: u32) -> Settlement {
    Settlement::new(id, 100, population, Arc::default())
}

/// Provinces 10..50 in one row; populations 5, 9, 9, 3 and warehouse capacity
/// 100. A founds nation 0 and B, C join it; D founds nation 1. Province 50 is
/// empty.
fn two_nations() -> World {
    let mut world = World::new(
        Map::new(5, 1, vec![Area::new(0.5, 0.5); 5], vec![10, 20, 30, 40, 50]),
        [10, 20, 30, 40, 50]
            .into_iter()
            .map(Province::new)
            .collect(),
    );
    world.add_settlement(10, settlement(A, 5)).unwrap();
    world
        .add_settlement_to_nation(FIRST, 20, settlement(B, 9))
        .unwrap();
    world
        .add_settlement_to_nation(FIRST, 30, settlement(C, 9))
        .unwrap();
    world.add_settlement(40, settlement(D, 3)).unwrap();
    world
}

fn subsidy(
    settlement_id: SettlementId,
    gold: u32,
    resources: &[(ResourceType, u32)],
) -> NationAction {
    NationAction::Subsidize {
        settlement_id,
        gold,
        resources: resources.iter().copied().collect(),
    }
}

/// JSON as an agent receives it: the text, parsed back. `serde_json::to_value`
/// would widen `f32` to `f64` (0.2 becomes 0.20000000298…); the text keeps 0.2.
fn json<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
}

fn error_of(outcome: &NationActionOutcome) -> &NationActionError {
    match outcome {
        NationActionOutcome::Rejected { error } => error,
        NationActionOutcome::Applied { effect } => panic!("applied: {effect:?}"),
    }
}

fn simulation_with_funds(gold: u32, wood: u32) -> Simulation {
    let mut world = two_nations();
    world.deposit_nation_gold(FIRST, gold).unwrap();
    world.deposit_nation_resource(FIRST, Wood, wood).unwrap();
    Simulation::new(world).unwrap()
}

// ---------------------------------------------------------------- ownership

#[test]
fn a_settlement_registered_alone_founds_a_nation_and_becomes_its_capital() {
    let world = two_nations();
    let first = world.nation(FIRST).unwrap();
    assert_eq!(first.capital(), A);
    assert_eq!(first.tax_rate(), Nation::BASE_TAX_RATE);
    assert_eq!((first.treasury(), first.capital_loss_turns()), (0, 0));
    assert_eq!(world.nation(SECOND).unwrap().capital(), D);
    assert_eq!(
        world.nations().len(),
        2,
        "joining settlements found nothing"
    );
    assert_eq!(world.settlement_owner(B), Some(FIRST));
    assert_eq!(world.settlement_owner(SettlementId(9)), None, "unknown ID");
    assert_eq!(world.province_owner(20), Ok(Some(FIRST)));
    assert_eq!(world.province_owner(50), Ok(None), "empty province");
    assert_eq!(
        world.province_owner(99),
        Err(WorldError::MissingProvince(99))
    );
    assert_eq!(world.nation_settlements(FIRST), Ok(vec![A, B, C]));
    assert_eq!(world.nation_stock_capacity(FIRST), Ok(1_500));
}

#[test]
fn every_registered_settlement_belongs_to_exactly_one_nation() {
    let world = two_nations();
    for settlement in world.settlements() {
        let owners = world
            .nations()
            .iter()
            .filter(|nation| {
                world
                    .nation_settlements(nation.id())
                    .unwrap()
                    .contains(&settlement.id())
            })
            .count();
        assert_eq!(owners, 1, "{:?}", settlement.id());
    }
}

#[test]
fn a_failed_registration_registers_nothing_and_founds_no_nation() {
    let mut world = two_nations();
    assert_eq!(
        world.add_settlement_to_nation(NationId(7), 50, settlement(SettlementId(5), 1)),
        Err(WorldError::MissingNation(NationId(7)))
    );
    assert!(world.settlement(SettlementId(5)).is_none());
    assert_eq!(world.province_settlement(50), Ok(None));
    assert_eq!(
        world.add_settlement(10, settlement(SettlementId(6), 1)),
        Err(WorldError::ProvinceOccupied {
            province_id: 10,
            settlement_id: A
        })
    );
    assert_eq!(world.nations().len(), 2);
}

#[test]
fn a_transfer_moves_ownership_and_capacity() {
    let mut world = two_nations();
    let change = world.transfer_settlement(B, SECOND).unwrap();
    assert_eq!(
        change,
        OwnershipChange {
            settlement_id: B,
            from: FIRST,
            to: SECOND,
            new_capital: None,
            extinct: false,
            gold_lost: 0,
            stock_lost: BTreeMap::new(),
        }
    );
    assert_eq!(world.nation_settlements(FIRST), Ok(vec![A, C]));
    assert_eq!(world.nation_settlements(SECOND), Ok(vec![B, D]));
    assert_eq!(world.province_owner(20), Ok(Some(SECOND)));
    assert_eq!(world.nation_stock_capacity(FIRST), Ok(1_000));
    assert_eq!(world.nation_stock_capacity(SECOND), Ok(1_000));
}

#[test]
fn invalid_transfers_change_nothing() {
    let mut world = two_nations();
    assert_eq!(
        world.transfer_settlement(B, FIRST),
        Err(WorldError::SettlementAlreadyOwned {
            settlement_id: B,
            nation_id: FIRST
        })
    );
    assert_eq!(
        world.transfer_settlement(B, NationId(7)),
        Err(WorldError::MissingNation(NationId(7)))
    );
    assert_eq!(
        world.transfer_settlement(SettlementId(9), FIRST),
        Err(WorldError::MissingSettlement(SettlementId(9)))
    );
    assert_eq!(world.nation_settlements(FIRST), Ok(vec![A, B, C]));
}

// ---------------------------------------------------------------- capital

#[test]
fn losing_the_capital_moves_it_to_the_largest_settlement_and_starts_the_penalty() {
    let mut world = two_nations();
    let change = world.transfer_settlement(A, SECOND).unwrap();
    // B and C both have population 9: the lower ID wins the tie.
    assert_eq!(change.new_capital, Some(B));
    let nation = world.nation(FIRST).unwrap();
    assert_eq!(nation.capital(), B);
    assert_eq!(
        nation.capital_loss_turns(),
        Nation::CAPITAL_LOSS_PENALTY_TURNS
    );
    // The receiving nation keeps its capital.
    assert_eq!(world.nation(SECOND).unwrap().capital(), D);
    assert_eq!(world.nation(SECOND).unwrap().capital_loss_turns(), 0);
}

#[test]
fn losing_another_settlement_keeps_the_capital() {
    let mut world = two_nations();
    let change = world.transfer_settlement(C, SECOND).unwrap();
    assert_eq!(change.new_capital, None);
    assert_eq!(world.nation(FIRST).unwrap().capital(), A);
    assert_eq!(world.nation(FIRST).unwrap().capital_loss_turns(), 0);
}

#[test]
fn the_capital_loss_penalty_counts_down_once_per_turn() {
    let mut world = two_nations();
    world.transfer_settlement(A, SECOND).unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    for expected in (0..Nation::CAPITAL_LOSS_PENALTY_TURNS).rev() {
        let report = simulation.step().unwrap();
        assert_eq!(report.nations[&FIRST].capital_loss_turns, expected);
        assert_eq!(
            simulation.observe_nation(FIRST).unwrap().capital_loss_turns,
            expected
        );
    }
}

// ---------------------------------------------------------------- stock and extinction

#[test]
fn the_stock_shares_one_capacity_that_follows_the_settlements() {
    let mut world = two_nations();
    // Three settlements: 1 500 units for all materials together.
    assert_eq!(world.nation(FIRST).unwrap().stock().capacity(), 1_500);
    assert_eq!(
        world
            .deposit_nation_resource(FIRST, Wood, 1_000)
            .unwrap()
            .accepted,
        1_000
    );
    let stone = world.deposit_nation_resource(FIRST, Stone, 700).unwrap();
    assert_eq!((stone.accepted, stone.rejected), (500, 200));
    assert_eq!(
        world
            .deposit_nation_resource(FIRST, Labour, 5)
            .unwrap()
            .rejected,
        5,
        "labour is never stored"
    );

    // Losing a settlement shrinks the capacity but destroys nothing.
    let change = world.transfer_settlement(C, SECOND).unwrap();
    assert!(change.stock_lost.is_empty());
    let stock = world.nation(FIRST).unwrap().stock();
    assert_eq!((stock.capacity(), stock.total_amount()), (1_000, 1_500));
    assert_eq!((stock.amount(Wood), stock.amount(Stone)), (1_000, 500));
    // Above the capacity the stock accepts nothing more.
    assert_eq!(
        world
            .deposit_nation_resource(FIRST, Grain, 1)
            .unwrap()
            .rejected,
        1
    );
    assert_eq!(world.nation(SECOND).unwrap().stock().capacity(), 1_000);
}

#[test]
fn invalid_tax_rates_are_rejected_by_validation() {
    let mut simulation = simulation_with_funds(0, 0);
    simulation
        .queue_nation_decision(NationDecision::new(
            FIRST,
            vec![
                NationAction::SetTaxRate { rate: 1.5 },
                NationAction::SetTaxRate { rate: -0.1 },
                NationAction::SetTaxRate { rate: 0.123_456 },
            ],
        ))
        .unwrap();
    let results = &simulation.step().unwrap().nation_decisions[0].results;
    assert_eq!(
        error_of(&results[0].outcome),
        &NationActionError::InvalidTaxRate { rate: 1.5 }
    );
    assert_eq!(
        error_of(&results[1].outcome),
        &NationActionError::InvalidTaxRate { rate: -0.1 }
    );
    assert!(results[2].is_applied(), "any fraction in 0..=1 is a rate");
    assert_eq!(
        simulation.observe_nation(FIRST).unwrap().tax_rate,
        0.123_456
    );
}

#[test]
fn a_nation_without_settlements_is_removed_with_its_treasury_and_stock() {
    let mut world = two_nations();
    world.deposit_nation_gold(SECOND, 70).unwrap();
    world.deposit_nation_resource(SECOND, Grain, 30).unwrap();

    let change = world.transfer_settlement(D, FIRST).unwrap();
    assert!(change.extinct);
    assert_eq!(change.gold_lost, 70);
    assert_eq!(change.stock_lost, BTreeMap::from([(Grain, 30)]));
    assert!(world.nation(SECOND).is_none());
    assert_eq!(world.nations().len(), 1);
    assert_eq!(
        world.nation_settlements(SECOND),
        Err(WorldError::MissingNation(SECOND))
    );
    assert_eq!(world.nation_settlements(FIRST), Ok(vec![A, B, C, D]));
    // A removed ID is never reused: the next settlement founds nation 2.
    world
        .add_settlement(50, settlement(SettlementId(5), 1))
        .unwrap();
    assert_eq!(world.settlement_owner(SettlementId(5)), Some(NationId(2)));
}

#[test]
fn extinct_nations_lose_their_pending_decisions_and_reject_new_ones() {
    let mut simulation = Simulation::new(two_nations()).unwrap();
    simulation
        .queue_nation_decision(NationDecision::new(
            SECOND,
            vec![NationAction::SetTaxRate { rate: 0.0 }],
        ))
        .unwrap();
    simulation.transfer_settlement(D, FIRST).unwrap();
    assert_eq!(
        simulation.queue_nation_decision(NationDecision::new(SECOND, vec![])),
        Err(WorldError::MissingNation(SECOND))
    );
    assert_eq!(
        simulation.observe_nation(SECOND),
        Err(WorldError::MissingNation(SECOND))
    );
    let report = simulation.step().unwrap();
    assert!(report.nation_decisions.is_empty());
    assert_eq!(
        report.nations.keys().copied().collect::<Vec<_>>(),
        vec![FIRST]
    );
    assert_eq!(
        simulation
            .observe_all_nations()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![FIRST]
    );
}

// ---------------------------------------------------------------- decisions

#[test]
fn subsidies_move_gold_and_materials_without_creating_or_destroying_any() {
    let mut simulation = simulation_with_funds(100, 80);
    let gold_before = simulation.settlement(B).unwrap().gold();
    simulation
        .queue_nation_decision(NationDecision::new(
            FIRST,
            vec![subsidy(B, 60, &[(Wood, 50)])],
        ))
        .unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.nation_decisions[0].results[0].outcome,
        NationActionOutcome::Applied {
            effect: NationActionEffect::SubsidyDelivered {
                settlement_id: B,
                gold: 60,
                delivered: BTreeMap::from([(Wood, 50)]),
                returned: BTreeMap::new(),
            }
        }
    );
    let nation = simulation.world().nation(FIRST).unwrap();
    let settlement = simulation.settlement(B).unwrap();
    assert_eq!(nation.treasury() + settlement.gold(), 100 + gold_before);
    assert_eq!(
        nation.stock().amount(Wood) + settlement.inventory().amount(Wood),
        80
    );
    assert_eq!((nation.treasury(), nation.stock().amount(Wood)), (40, 30));
}

#[test]
fn materials_that_do_not_fit_the_warehouse_stay_in_the_national_stock() {
    let mut simulation = simulation_with_funds(0, 300);
    // Warehouse capacity is 100.
    simulation
        .queue_nation_decision(NationDecision::new(
            FIRST,
            vec![subsidy(C, 0, &[(Wood, 130)])],
        ))
        .unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.nation_decisions[0].results[0].outcome,
        NationActionOutcome::Applied {
            effect: NationActionEffect::SubsidyDelivered {
                settlement_id: C,
                gold: 0,
                delivered: BTreeMap::from([(Wood, 100)]),
                returned: BTreeMap::from([(Wood, 30)]),
            }
        }
    );
    assert_eq!(
        simulation
            .world()
            .nation(FIRST)
            .unwrap()
            .stock()
            .amount(Wood),
        200
    );
    assert_eq!(
        simulation.settlement(C).unwrap().inventory().amount(Wood),
        100
    );
}

#[test]
fn subsidies_of_one_decision_share_the_treasury_and_stock() {
    let mut simulation = simulation_with_funds(100, 50);
    simulation
        .queue_nation_decision(NationDecision::new(
            FIRST,
            vec![
                subsidy(A, 70, &[(Wood, 30)]),
                subsidy(B, 40, &[]),
                subsidy(B, 0, &[(Wood, 25)]),
                subsidy(C, 30, &[(Wood, 20)]),
            ],
        ))
        .unwrap();
    let results = &simulation.step().unwrap().nation_decisions[0].results;
    assert!(results[0].is_applied());
    assert_eq!(
        error_of(&results[1].outcome),
        &NationActionError::InsufficientTreasury {
            requested: 40,
            available: 30
        }
    );
    assert_eq!(
        error_of(&results[2].outcome),
        &NationActionError::InsufficientStock {
            resource: Wood,
            requested: 25,
            available: 20
        }
    );
    assert!(results[3].is_applied(), "rejected actions claim nothing");
    let nation = simulation.world().nation(FIRST).unwrap();
    assert_eq!((nation.treasury(), nation.stock().amount(Wood)), (0, 0));
}

#[test]
fn subsidies_go_only_to_own_settlements_and_must_carry_something() {
    let mut simulation = simulation_with_funds(10, 10);
    simulation
        .queue_nation_decision(NationDecision::new(
            FIRST,
            vec![
                subsidy(D, 5, &[]),
                subsidy(SettlementId(9), 5, &[]),
                subsidy(A, 0, &[]),
                subsidy(A, 0, &[(Wood, 0)]),
                subsidy(A, 0, &[(Labour, 1)]),
                subsidy(A, 0, &[(Labour, 0), (Wood, 1)]),
            ],
        ))
        .unwrap();
    let results = &simulation.step().unwrap().nation_decisions[0].results;
    let errors: Vec<_> = results
        .iter()
        .map(|result| error_of(&result.outcome).clone())
        .collect();
    assert_eq!(
        errors,
        vec![
            NationActionError::SettlementNotOwned { settlement_id: D },
            NationActionError::UnknownSettlement {
                settlement_id: SettlementId(9)
            },
            NationActionError::EmptySubsidy,
            NationActionError::EmptySubsidy,
            NationActionError::LabourIsNotTransferable,
            NationActionError::LabourIsNotTransferable,
        ]
    );
    let nation = simulation.world().nation(FIRST).unwrap();
    assert_eq!((nation.treasury(), nation.stock().amount(Wood)), (10, 10));
}

#[test]
fn a_subsidy_that_would_overflow_settlement_gold_is_rejected() {
    let mut world = two_nations();
    world.deposit_nation_gold(FIRST, 10).unwrap();
    let mut rich = settlement(SettlementId(5), 1);
    rich.deposit_gold(u32::MAX - 5);
    world.add_settlement_to_nation(FIRST, 50, rich).unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    simulation
        .queue_nation_decision(NationDecision::new(
            FIRST,
            vec![
                subsidy(SettlementId(5), 4, &[]),
                subsidy(SettlementId(5), 2, &[]),
            ],
        ))
        .unwrap();
    let results = &simulation.step().unwrap().nation_decisions[0].results;
    assert!(results[0].is_applied());
    assert_eq!(
        error_of(&results[1].outcome),
        &NationActionError::SettlementTreasuryFull {
            settlement_id: SettlementId(5)
        }
    );
}

#[test]
fn the_tax_rate_is_policy_set_once_per_decision_and_seen_by_own_settlements() {
    let mut simulation = simulation_with_funds(0, 0);
    simulation
        .queue_nation_decision(NationDecision::new(
            FIRST,
            vec![
                NationAction::SetTaxRate { rate: 0.25 },
                NationAction::SetTaxRate { rate: 0.0 },
            ],
        ))
        .unwrap();
    let report = simulation.step().unwrap();
    let results = &report.nation_decisions[0].results;
    assert_eq!(
        results[0].outcome,
        NationActionOutcome::Applied {
            effect: NationActionEffect::TaxRateSet { rate: 0.25 }
        }
    );
    assert_eq!(
        error_of(&results[1].outcome),
        &NationActionError::Conflict { with_index: 0 }
    );
    assert_eq!(results[1].kind, NationActionKind::SetTaxRate);
    assert_eq!(report.nations[&FIRST].tax_rate, 0.25);
    assert_eq!(
        simulation.observe_nation(SECOND).unwrap().tax_rate,
        Nation::BASE_TAX_RATE
    );

    let policy = simulation.observe(B).unwrap().nation;
    assert_eq!(policy.nation_id, FIRST);
    assert_eq!(policy.tax_rate, 0.25);
    assert_eq!((policy.capital, policy.is_capital), (A, false));
    assert!(simulation.observe(A).unwrap().nation.is_capital);
    assert_eq!(
        simulation.observe(D).unwrap().nation.tax_rate,
        Nation::BASE_TAX_RATE
    );
}

#[test]
fn nation_decisions_run_before_settlement_decisions_in_the_same_turn() {
    // B has no gold; its caravan to D buys Wood for 20 gold. The nation's subsidy
    // of the same turn funds it, because nations decide first.
    let mut simulation = simulation_with_funds(20, 0);
    assert_eq!(simulation.settlement(B).unwrap().gold(), 0);
    simulation
        .queue_decision(SettlementDecision::new(
            B,
            vec![SettlementAction::OrderCaravan {
                receiver_id: D,
                offers: vec![OfferSpec {
                    resource: Wood,
                    side: OfferSide::Buy,
                    units: 4,
                    price_per_unit: 5,
                }],
            }],
        ))
        .unwrap();
    simulation
        .queue_nation_decision(NationDecision::new(FIRST, vec![subsidy(B, 20, &[])]))
        .unwrap();
    let report = simulation.step().unwrap();
    assert!(report.nation_decisions[0].results[0].is_applied());
    assert!(
        report.decisions[0].results[0].is_applied(),
        "{:?}",
        report.decisions[0]
    );
    assert_eq!(report.settlements[&B].trade.caravans_dispatched.len(), 1);
}

#[test]
fn unknown_nations_cannot_queue_decisions() {
    let mut simulation = simulation_with_funds(0, 0);
    assert_eq!(
        simulation.queue_nation_decision(NationDecision::new(NationId(5), vec![])),
        Err(WorldError::MissingNation(NationId(5)))
    );
}

#[test]
fn nation_outcomes_serialize_with_tags() {
    let mut simulation = simulation_with_funds(5, 0);
    simulation
        .queue_nation_decision(
            NationDecision::from_json(
                r#"{"nation_id": 0, "actions": [
                    {"type": "subsidize", "settlement_id": 4, "gold": 1},
                    {"type": "set_tax_rate", "rate": 0.2}
                ]}"#,
            )
            .unwrap(),
        )
        .unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        json(&report.nation_decisions),
        serde_json::json!([{
            "nation_id": 0,
            "results": [
                {"index": 0, "kind": "subsidize", "outcome": {"status": "rejected",
                    "error": {"reason": "settlement_not_owned", "settlement_id": 4}}},
                {"index": 1, "kind": "set_tax_rate", "outcome": {"status": "applied",
                    "effect": {"effect": "tax_rate_set", "rate": 0.2}}}
            ]
        }])
    );
}

// ---------------------------------------------------------------- report and observation

#[test]
fn every_nation_gets_its_own_end_of_turn_report() {
    let mut simulation = simulation_with_funds(30, 40);
    simulation
        .queue_nation_decision(NationDecision::new(FIRST, vec![subsidy(B, 10, &[])]))
        .unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.nations.keys().copied().collect::<Vec<_>>(),
        vec![FIRST, SECOND]
    );
    let first = &report.nations[&FIRST];
    assert_eq!(first.capital, A);
    assert_eq!(first.settlements, vec![A, B, C]);
    assert_eq!(first.closing_treasury, 20);
    assert_eq!(first.closing_stock, BTreeMap::from([(Wood, 40)]));
    assert_eq!(
        simulation.observe_nation(FIRST).unwrap().last_turn.as_ref(),
        Some(first)
    );
    let json = json(first);
    assert_eq!(json["tax_rate"], serde_json::json!(0.1));
    assert_eq!(json["closing_stock"], serde_json::json!({"Wood": 40}));
}

#[test]
fn a_nation_observes_its_own_state_and_only_ids_of_other_nations() {
    let mut world = two_nations();
    world.deposit_nation_gold(FIRST, 12).unwrap();
    world.deposit_nation_resource(FIRST, Stone, 7).unwrap();
    let simulation = Simulation::new(world).unwrap();
    let observation = simulation.observe_nation(FIRST).unwrap();
    assert_eq!(observation.turn, 0);
    assert_eq!((observation.nation_id, observation.capital), (FIRST, A));
    assert_eq!(observation.treasury, 12);
    assert_eq!(observation.stock, BTreeMap::from([(Stone, 7)]));
    assert_eq!(
        (observation.stock_total, observation.stock_capacity),
        (7, 1_500)
    );
    assert_eq!(
        observation
            .settlements
            .iter()
            .map(|settlement| (
                settlement.settlement_id,
                settlement.province_id,
                settlement.population
            ))
            .collect::<Vec<_>>(),
        vec![(A, 10, 5), (B, 20, 9), (C, 30, 9)]
    );
    assert_eq!(observation.settlements[0].free_capacity, 100);
    assert_eq!(observation.other_nations, vec![SECOND]);
    assert!(observation.last_turn.is_none(), "no turn completed yet");
    let json = observation.to_json();
    assert_eq!(
        serde_json::from_str::<NationObservation>(&json).unwrap(),
        observation
    );
}

#[test]
fn another_nations_treasury_and_stock_do_not_change_an_observation() {
    let observe = |gold: u32, stone: u32| {
        let mut world = two_nations();
        world.deposit_nation_gold(SECOND, gold).unwrap();
        world.deposit_nation_resource(SECOND, Stone, stone).unwrap();
        let simulation = Simulation::new(world).unwrap();
        (
            simulation.observe_nation(FIRST).unwrap(),
            simulation.observe(A).unwrap(),
        )
    };
    assert_eq!(observe(0, 0), observe(400, 300));
}

#[test]
fn settlements_see_the_owner_of_trade_partners() {
    let simulation = Simulation::new(two_nations()).unwrap();
    let partners: Vec<_> = simulation
        .observe(A)
        .unwrap()
        .trade_partners
        .into_iter()
        .map(|partner| (partner.settlement_id, partner.nation_id))
        .collect();
    assert_eq!(partners, vec![(B, FIRST), (C, FIRST), (D, SECOND)]);
}

#[test]
fn turns_without_decisions_leave_national_funds_unchanged() {
    let mut simulation = simulation_with_funds(30, 40);
    for _ in 0..3 {
        simulation.step().unwrap();
    }
    let nation = simulation.world().nation(FIRST).unwrap();
    assert_eq!((nation.treasury(), nation.stock().amount(Wood)), (30, 40));
}
