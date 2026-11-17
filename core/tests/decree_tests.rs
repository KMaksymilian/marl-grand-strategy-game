//! Decrees (gameplay specification §7): slots, delayed activation, minimum
//! duration, cooldown, per-settlement upkeep, lapse when unpaid, stacking of
//! effects in their owning systems (production, construction cost, caravan
//! speed) and the one-shot capital relocation with its refund.
use simulation_core::buildings::Building;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::economy::ResourceType::{self, Grain, Wood};
use simulation_core::ids::{BuildingId, BuildingTypeId, NationId, SettlementId};
use simulation_core::nation::TaxRate;
use simulation_core::nation::decree::{
    CapitalRelocation, Decree, DecreeState, RelocationFailure, RelocationOutcome,
};
use simulation_core::settlement::Settlement;
use simulation_core::settlement::decision::{OfferSide, OfferSpec};
use simulation_core::settlement::projects::ProjectCost;
use simulation_core::simulation::{
    NationAction, NationActionEffect, NationActionError, NationActionOutcome, NationDecision,
    NationObservation, SettlementAction, SettlementDecision, Simulation, SimulationTurnReport,
};
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::{Map, Position, Province, World, terrain::area::Area};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

const A: SettlementId = SettlementId(1);
const B: SettlementId = SettlementId(2);
const C: SettlementId = SettlementId(3);
const FIRST: NationId = NationId(0);
const SECOND: NationId = NationId(1);

fn at(x: i32) -> Position {
    Position { x, y: 0 }
}

fn producer(id: &str, output: ResourceType, cost: u32) -> BuildingDefinition {
    BuildingDefinition::new(
        id,
        id,
        1,
        10,
        vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(output, 100)],
        }],
        ProjectCost {
            needed_resources: HashMap::from([(Wood, cost)]),
            turn_cost: 3,
        },
    )
    .unwrap()
}

/// Provinces `10 10 10 20 20 30 30`. A founds nation 0 (treasury 1 000) with a
/// farm making 100 Grain; B joins it with a camp making 100 Wood; C founds
/// nation 1. The catalog has the farm (50 Wood to build). Tax is 0% so that
/// production is easy to read.
fn world(treasury: u32) -> World {
    let mut world = World::new(
        Map::new(
            7,
            1,
            vec![Area::new(0.5, 0.5); 7],
            vec![10, 10, 10, 20, 20, 30, 30],
        ),
        [10, 20, 30].into_iter().map(Province::new).collect(),
    );
    let mut a = Settlement::new(A, 10_000, 100, Arc::default());
    a.deposit(Wood, 500);
    world.add_settlement_at(10, at(0), a).unwrap();
    world
        .add_settlement_to_nation(FIRST, 20, Settlement::new(B, 10_000, 50, Arc::default()))
        .unwrap();
    world
        .add_settlement_at(30, at(5), Settlement::new(C, 10_000, 40, Arc::default()))
        .unwrap();
    world.deposit_nation_gold(FIRST, treasury).unwrap();
    let mut catalog = BuildingCatalog::default();
    catalog.register(producer("farm", Grain, 50));
    let farm = catalog.get_blueprint("farm").unwrap();
    world.set_building_catalog(catalog);
    let mut finder = PathFinder::new(7, 1);
    world
        .add_building(A, Building::new(BuildingId(1), at(1), farm), &mut finder)
        .unwrap();
    world
        .add_building(
            B,
            Building::new(BuildingId(1), at(4), Arc::new(producer("camp", Wood, 50))),
            &mut finder,
        )
        .unwrap();
    world
}

/// Staffs both producers in the first turn and sets nation 0's tax to 0%.
fn simulation(treasury: u32) -> Simulation {
    let mut simulation = Simulation::new(world(treasury)).unwrap();
    for settlement in [A, B] {
        simulation
            .queue_decision(SettlementDecision::new(
                settlement,
                vec![SettlementAction::SetBuildingWorkers {
                    building_id: BuildingId(1),
                    workers: 10,
                }],
            ))
            .unwrap();
    }
    decide(
        &mut simulation,
        vec![NationAction::SetTaxRate {
            rate: TaxRate::ZERO,
        }],
    );
    simulation
}

fn decide(simulation: &mut Simulation, actions: Vec<NationAction>) {
    simulation
        .queue_nation_decision(NationDecision::new(FIRST, actions))
        .unwrap();
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

/// Outcomes of nation 0's decision in this report (the last one if several).
fn outcomes(report: &SimulationTurnReport) -> Vec<NationActionOutcome> {
    report
        .nation_decisions
        .iter()
        .rfind(|outcome| outcome.nation_id == FIRST)
        .map(|outcome| {
            outcome
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

fn produced(
    report: &SimulationTurnReport,
    settlement: SettlementId,
    resource: ResourceType,
) -> u64 {
    report.settlements[&settlement].inventory.produced[&resource]
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

// ---------------------------------------------------------------- lifecycle

#[test]
fn a_decree_works_from_the_turn_after_it_is_enacted_and_costs_upkeep_while_active() {
    let mut simulation = simulation(1_000);
    decide(&mut simulation, vec![enact(Decree::AgriculturalDecree)]);
    let report = simulation.step().unwrap();
    assert!(outcomes(&report).contains(&NationActionOutcome::Applied {
        effect: NationActionEffect::DecreeEnacted {
            decree: Decree::AgriculturalDecree,
            // 10 gold × 2 settlements × 1.
            upkeep: 20,
        }
    }));
    // Turn 1: pending, no effect and no cost.
    assert_eq!(
        state(&simulation, Decree::AgriculturalDecree),
        Some(DecreeState::Active { turns_active: 0 })
    );
    assert_eq!(produced(&report, A, Grain), 100);
    assert_eq!(produced(&report, B, Wood), 100);
    assert!(report.nations[&FIRST].decrees.active.is_empty());
    assert_eq!(treasury(&simulation), 1_000);

    // Turn 2: active, +20% food and −10% building materials; upkeep paid.
    let report = simulation.step().unwrap();
    assert_eq!(produced(&report, A, Grain), 120);
    assert_eq!(produced(&report, B, Wood), 90);
    let decrees = &report.nations[&FIRST].decrees;
    assert_eq!(decrees.active, vec![Decree::AgriculturalDecree]);
    assert_eq!(decrees.upkeep_paid, 20);
    assert_eq!(treasury(&simulation), 980);
    // Another nation is not affected.
    assert!(report.nations[&SECOND].decrees.active.is_empty());
}

#[test]
fn a_decree_can_be_revoked_only_after_five_active_turns() {
    let mut simulation = simulation(1_000);
    decide(&mut simulation, vec![enact(Decree::Games)]);
    simulation.step().unwrap();
    // Turns 2..=6 are its first five active turns.
    for turns_left in (1..=5).rev() {
        decide(&mut simulation, vec![revoke(Decree::Games)]);
        let report = simulation.step().unwrap();
        assert_eq!(
            error(&outcomes(&report)[0]),
            NationActionError::DecreeMinimumDuration {
                decree: Decree::Games,
                turns_left
            }
        );
        assert_eq!(report.nations[&FIRST].decrees.active, vec![Decree::Games]);
    }
    decide(&mut simulation, vec![revoke(Decree::Games)]);
    let report = simulation.step().unwrap();
    assert_eq!(
        outcomes(&report)[0],
        NationActionOutcome::Applied {
            effect: NationActionEffect::DecreeRevoked {
                decree: Decree::Games,
                cooldown_turns: 5
            }
        }
    );
    assert!(
        report.nations[&FIRST].decrees.active.is_empty(),
        "a revoked decree costs and does nothing in the same turn"
    );
    // Five paid turns: 10 × 2 settlements × 4 each.
    assert_eq!(treasury(&simulation), 1_000 - 5 * 80);
}

#[test]
fn a_revoked_decree_can_be_enacted_again_after_five_turns() {
    let mut simulation = simulation(1_000);
    decide(&mut simulation, vec![enact(Decree::FreeTrade)]);
    for _ in 0..6 {
        simulation.step().unwrap();
    }
    // Turn 7: revoke, and the same decree cannot come back in this decision.
    decide(
        &mut simulation,
        vec![revoke(Decree::FreeTrade), enact(Decree::FreeTrade)],
    );
    let report = simulation.step().unwrap();
    assert!(matches!(
        outcomes(&report)[0],
        NationActionOutcome::Applied { .. }
    ));
    assert_eq!(
        error(&outcomes(&report)[1]),
        NationActionError::Conflict { with_index: 0 }
    );
    // Turns 8..=11: still cooling down.
    for turns_left in (1..=4).rev() {
        decide(&mut simulation, vec![enact(Decree::FreeTrade)]);
        let report = simulation.step().unwrap();
        assert_eq!(
            error(&outcomes(&report)[0]),
            NationActionError::DecreeOnCooldown {
                decree: Decree::FreeTrade,
                turns_left
            }
        );
    }
    assert_eq!(state(&simulation, Decree::FreeTrade), None);
    decide(&mut simulation, vec![enact(Decree::FreeTrade)]);
    let report = simulation.step().unwrap();
    assert!(matches!(
        outcomes(&report)[0],
        NationActionOutcome::Applied { .. }
    ));
}

#[test]
fn pending_and_active_decrees_use_the_two_slots_and_a_revocation_frees_one() {
    let mut simulation = simulation(10_000);
    decide(
        &mut simulation,
        vec![
            enact(Decree::Games),
            enact(Decree::Games),
            enact(Decree::Discipline),
            enact(Decree::Protectionism),
        ],
    );
    let report = simulation.step().unwrap();
    let results = outcomes(&report);
    assert!(matches!(results[0], NationActionOutcome::Applied { .. }));
    assert_eq!(
        error(&results[1]),
        NationActionError::Conflict { with_index: 0 }
    );
    assert!(matches!(results[2], NationActionOutcome::Applied { .. }));
    assert_eq!(
        error(&results[3]),
        NationActionError::NoFreeDecreeSlot { slots: 2 }
    );
    // Pending decrees cannot be revoked, and are not enacted twice.
    decide(
        &mut simulation,
        vec![revoke(Decree::Games), enact(Decree::Discipline)],
    );
    let results = outcomes(&simulation.step().unwrap());
    assert_eq!(
        error(&results[0]),
        NationActionError::DecreeMinimumDuration {
            decree: Decree::Games,
            turns_left: 5
        }
    );
    assert_eq!(
        error(&results[1]),
        NationActionError::DecreeAlreadyEnacted {
            decree: Decree::Discipline
        }
    );
    for _ in 0..4 {
        simulation.step().unwrap();
    }
    // Revoking frees a slot for another decree in the same decision.
    decide(
        &mut simulation,
        vec![revoke(Decree::Games), enact(Decree::Protectionism)],
    );
    let results = outcomes(&simulation.step().unwrap());
    assert!(
        results
            .iter()
            .all(|outcome| matches!(outcome, NationActionOutcome::Applied { .. }))
    );
    let observation = simulation.observe_nation(FIRST).unwrap();
    assert_eq!(
        (observation.decrees.slots, observation.decrees.used_slots),
        (2, 2)
    );
}

#[test]
fn a_decree_the_treasury_cannot_pay_lapses_into_cooldown() {
    // Games cost 10 × 2 × 4 = 80 per turn; the treasury holds 100.
    let mut simulation = simulation(100);
    decide(&mut simulation, vec![enact(Decree::Games)]);
    simulation.step().unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(report.nations[&FIRST].decrees.upkeep_paid, 80);
    let report = simulation.step().unwrap();
    let decrees = &report.nations[&FIRST].decrees;
    assert_eq!(decrees.lapsed, vec![Decree::Games]);
    assert!(decrees.active.is_empty());
    assert_eq!(treasury(&simulation), 20, "an unpaid decree takes nothing");
    assert_eq!(
        state(&simulation, Decree::Games),
        Some(DecreeState::Cooldown { turns_left: 4 })
    );
}

#[test]
fn upkeep_scales_with_the_number_of_settlements() {
    let mut world = world(1_000);
    world.transfer_settlement(C, FIRST).unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    decide(&mut simulation, vec![enact(Decree::GreatConstruction)]);
    let report = simulation.step().unwrap();
    assert!(outcomes(&report).contains(&NationActionOutcome::Applied {
        effect: NationActionEffect::DecreeEnacted {
            decree: Decree::GreatConstruction,
            upkeep: 60
        }
    }));
    let report = simulation.step().unwrap();
    assert_eq!(report.nations[&FIRST].decrees.upkeep_paid, 60);
}

// ---------------------------------------------------------------- effects

#[test]
fn production_changes_of_several_decrees_stack() {
    let mut simulation = simulation(1_000);
    decide(
        &mut simulation,
        vec![
            enact(Decree::AgriculturalDecree),
            enact(Decree::Mobilization),
        ],
    );
    simulation.step().unwrap();
    let report = simulation.step().unwrap();
    // Grain +20% − 15%; Wood −10% − 15%.
    assert_eq!(produced(&report, A, Grain), 105);
    assert_eq!(produced(&report, B, Wood), 75);
    for report in report.settlements.values() {
        assert!(report.inventory.is_balanced());
    }
}

#[test]
fn production_tax_applies_to_the_changed_output() {
    let mut simulation = simulation(1_000);
    decide(&mut simulation, vec![enact(Decree::AgriculturalDecree)]);
    simulation.step().unwrap();
    decide(
        &mut simulation,
        vec![NationAction::SetTaxRate {
            rate: TaxRate::BASE,
        }],
    );
    let report = simulation.step().unwrap();
    let tax = &report.settlements[&A]
        .taxation
        .as_ref()
        .unwrap()
        .production_tax;
    assert_eq!(tax[&Grain], 12, "10% of 120");
}

#[test]
fn great_construction_lowers_the_material_cost_of_new_projects() {
    let mut simulation = simulation(1_000);
    decide(&mut simulation, vec![enact(Decree::GreatConstruction)]);
    simulation.step().unwrap();
    // Turn 2: the decree works for nation 0 only. Both start a farm (50 Wood).
    for (settlement, x) in [(A, 2), (C, 6)] {
        simulation
            .queue_decision(SettlementDecision::new(
                settlement,
                vec![SettlementAction::Construct {
                    building_type: BuildingTypeId::new("farm"),
                    position: at(x),
                }],
            ))
            .unwrap();
    }
    let report = simulation.step().unwrap();
    assert!(
        report
            .decisions
            .iter()
            .all(|decision| decision.results[0].is_applied())
    );
    for (settlement, total) in [(A, 40), (C, 50)] {
        // Paid this turn plus what is still due is the project's whole cost.
        let paid = report.settlements[&settlement]
            .inventory
            .project_consumed
            .get(&Wood)
            .copied()
            .unwrap_or(0);
        let due: u64 = simulation
            .settlement(settlement)
            .unwrap()
            .active_projects()
            .iter()
            .filter_map(|project| project.remaining_cost().needed_resources.get(&Wood))
            .map(|&amount| u64::from(amount))
            .sum();
        assert_eq!(paid + due, total, "{settlement:?}");
    }
}

#[test]
fn free_trade_speeds_up_the_nations_caravans() {
    let mut simulation = simulation(1_000);
    decide(&mut simulation, vec![enact(Decree::FreeTrade)]);
    simulation.step().unwrap();
    let caravan = |receiver| {
        SettlementDecision::new(
            A,
            vec![SettlementAction::OrderCaravan {
                receiver_id: receiver,
                offers: vec![OfferSpec {
                    resource: Wood,
                    side: OfferSide::Sell,
                    units: 1,
                    price_per_unit: 1,
                }],
            }],
        )
    };
    simulation.queue_decision(caravan(C)).unwrap();
    simulation.step().unwrap();
    let speeds: Vec<_> = simulation
        .world()
        .caravans
        .iter()
        .map(|caravan| (caravan.sender_id, caravan.speed()))
        .collect();
    assert_eq!(speeds, vec![(A, 12)], "10 steps + 25%");
}

// ---------------------------------------------------------------- relocation

#[test]
fn the_capital_moves_one_turn_after_the_relocation_is_paid() {
    let mut simulation = simulation(1_000);
    decide(&mut simulation, vec![relocate(B)]);
    let report = simulation.step().unwrap();
    assert!(outcomes(&report).contains(&NationActionOutcome::Applied {
        effect: NationActionEffect::CapitalRelocationOrdered {
            settlement_id: B,
            cost: 200
        }
    }));
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
    let mut simulation = simulation(1_000);
    decide(&mut simulation, vec![relocate(B)]);
    simulation.step().unwrap();
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
    decide(&mut simulation, vec![relocate(B)]);
    simulation.step().unwrap();
    simulation.transfer_settlement(B, SECOND).unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.nations[&FIRST].capital_relocation,
        Some(RelocationOutcome::Refunded {
            target: B,
            reason: RelocationFailure::NotOwned,
            refund: 200
        })
    );
    assert_eq!(treasury(&simulation), 1_000);
}

#[test]
fn relocation_orders_are_checked_and_use_no_decree_slot() {
    let mut simulation = simulation(250);
    simulation.set_settlement_under_attack(B, true).unwrap();
    decide(
        &mut simulation,
        vec![
            relocate(A),
            relocate(C),
            relocate(SettlementId(9)),
            relocate(B),
        ],
    );
    let errors: Vec<_> = outcomes(&simulation.step().unwrap())
        .iter()
        .map(error)
        .collect();
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
    // Two decrees plus a relocation: all accepted. A second relocation conflicts.
    decide(
        &mut simulation,
        vec![
            enact(Decree::Discipline),
            enact(Decree::Protectionism),
            relocate(B),
            relocate(B),
        ],
    );
    let results = outcomes(&simulation.step().unwrap());
    assert!(
        results[..3]
            .iter()
            .all(|outcome| matches!(outcome, NationActionOutcome::Applied { .. }))
    );
    assert_eq!(
        error(&results[3]),
        NationActionError::Conflict { with_index: 2 }
    );
    assert_eq!(treasury(&simulation), 50);
}

#[test]
fn relocation_needs_the_full_cost_after_earlier_subsidies() {
    let mut simulation = simulation(250);
    decide(
        &mut simulation,
        vec![
            NationAction::Subsidize {
                settlement_id: B,
                gold: 100,
                resources: BTreeMap::new(),
            },
            relocate(B),
        ],
    );
    let results = outcomes(&simulation.step().unwrap());
    assert_eq!(
        error(&results[1]),
        NationActionError::InsufficientTreasury {
            requested: 200,
            available: 150
        }
    );
}

// ---------------------------------------------------------------- observation

#[test]
fn nations_and_settlements_observe_decrees() {
    let mut simulation = simulation(1_000);
    decide(
        &mut simulation,
        vec![enact(Decree::AgriculturalDecree), relocate(B)],
    );
    simulation.step().unwrap();
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
    assert_eq!(decrees.upkeep[&Decree::Games], 80);
    assert_eq!(decrees.upkeep.len(), Decree::ALL.len());
    assert_eq!(decrees.relocation_cost, 200);
    assert!((decrees.effects.production_change(Grain) - 0.2).abs() < 1e-9);
    let json = observation.to_json();
    assert_eq!(
        serde_json::from_str::<NationObservation>(&json).unwrap(),
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
fn decree_actions_and_reports_serialize_in_snake_case() {
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
    let json = serde_json::to_value(&report.nation_decisions[1]).unwrap();
    assert_eq!(
        json["results"][0]["outcome"]["effect"],
        serde_json::json!({"effect": "decree_enacted", "decree": "great_construction", "upkeep": 40})
    );
    let report = simulation.step().unwrap();
    let nation = serde_json::to_value(&report.nations[&FIRST]).unwrap();
    assert_eq!(
        nation["capital_relocation"],
        serde_json::json!({"outcome": "completed", "from": 1, "to": 2})
    );
    assert_eq!(
        nation["decrees"]["active"],
        serde_json::json!(["great_construction"])
    );
}
