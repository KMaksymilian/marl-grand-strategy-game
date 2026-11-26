//! War on the map (gameplay specification §11): marches and access rules,
//! settlements under attack, battles with garrisons, walls and seeded
//! variance, retreats, conquest and withdrawals after a peace.
mod common;
use common::material;
use simulation_core::economy::ResourceCatalog;
use simulation_core::ids::{ArmyId, NationId, SettlementId};
use simulation_core::military::{BattleReport, BattleSide, MarchOutcome, UnitCatalog, UnitTypeId};
use simulation_core::nation::decree::DecreeId;
use simulation_core::nation::diplomacy::Treaty;
use simulation_core::settlement::Settlement;
use simulation_core::settlement::decision::{OfferSide, OfferSpec};
use simulation_core::simulation::{
    ActionError, ActionOutcome, NationAction, NationActionEffect, NationActionError,
    NationActionOutcome, NationDecision, SettlementAction, SettlementDecision, Simulation,
    SimulationTurnReport,
};
use simulation_core::world::{Map, Province, World, terrain::area::Area};
use std::sync::Arc;

const A: SettlementId = SettlementId(1);
const B: SettlementId = SettlementId(2);
const C: SettlementId = SettlementId(3);
const D: SettlementId = SettlementId(4);
const FIRST: NationId = NationId(0);
const SECOND: NationId = NationId(1);
const THIRD: NationId = NationId(2);

/// Provinces in a row: 10 | 20 | 50 | 30 | 40. A (5000 people) founds nation 0
/// in 10 and B (1000) joins it in 20; 50 has no settlement; C founds nation 1
/// in 30 with `c_population` people; D founds nation 2 in 40. Nation 0 holds
/// 10 000 gold and 500 grain (half of its stock capacity). Battles have no random variance unless
/// `variance`.
fn world(c_population: u32, variance: bool) -> World {
    let mut world = World::new(
        Map::new(5, 1, vec![Area::new(0.5, 0.5); 5], vec![10, 20, 50, 30, 40]),
        [10, 20, 50, 30, 40]
            .into_iter()
            .map(Province::new)
            .collect(),
    );
    world
        .add_settlement(10, Settlement::new(A, 100, 5_000, Arc::default()))
        .unwrap();
    world
        .add_settlement_to_nation(FIRST, 20, Settlement::new(B, 100, 1_000, Arc::default()))
        .unwrap();
    world
        .add_settlement(30, Settlement::new(C, 100, c_population, Arc::default()))
        .unwrap();
    world
        .add_settlement(40, Settlement::new(D, 100, 1_000, Arc::default()))
        .unwrap();
    world.deposit_nation_gold(FIRST, 10_000).unwrap();
    world
        .deposit_nation_resource(FIRST, material("grain"), 500)
        .unwrap();
    if !variance {
        let source = include_str!("../../units.toml")
            .replace("strength_variance = 0.1", "strength_variance = 0.0");
        let catalog = UnitCatalog::from_toml(&source, ResourceCatalog::standard()).unwrap();
        world.set_unit_catalog(Arc::new(catalog)).unwrap();
    }
    world
}

fn simulation(c_population: u32) -> Simulation {
    Simulation::new(world(c_population, false)).unwrap()
}

fn act(
    simulation: &mut Simulation,
    nation: NationId,
    actions: Vec<NationAction>,
) -> SimulationTurnReport {
    simulation
        .queue_nation_decision(NationDecision::new(nation, actions))
        .unwrap();
    simulation.step().unwrap()
}

fn outcome(report: &SimulationTurnReport, nation: NationId) -> NationActionOutcome {
    report
        .nation_decisions
        .iter()
        .rfind(|decision| decision.nation_id == nation)
        .unwrap()
        .results[0]
        .outcome
        .clone()
}

fn war(target: NationId) -> NationAction {
    NationAction::DeclareWar { target }
}

fn march(army_id: ArmyId, province_id: usize) -> NationAction {
    NationAction::MoveArmy {
        army_id,
        province_id,
    }
}

/// Recruits units of one type in A and waits until they are ready.
fn raise(simulation: &mut Simulation, unit: &str, units: u32) -> ArmyId {
    act(
        simulation,
        FIRST,
        vec![NationAction::RecruitUnits {
            settlement_id: A,
            unit: UnitTypeId::new(unit).unwrap(),
            units,
        }],
    );
    let report = simulation.step().unwrap();
    report.nations[&FIRST].military.recruits_joined[0].army_id
}

fn battles(report: &SimulationTurnReport, nation: NationId) -> Vec<BattleReport> {
    report.nations[&nation].military.battles.clone()
}

fn province_of(simulation: &Simulation, army: ArmyId) -> Option<usize> {
    simulation.world().army(army).map(|army| army.province())
}

// ---------------------------------------------------------------- marches

#[test]
fn armies_may_enter_only_own_allied_enemy_and_empty_provinces() {
    let mut simulation = simulation(1_000);
    let army = raise(&mut simulation, "levy", 2);
    let no_route = |province_id| NationActionOutcome::Rejected {
        error: NationActionError::NoRoute {
            army_id: army,
            province_id,
        },
    };
    // Nation 1 is neutral: its province is closed.
    let report = act(&mut simulation, FIRST, vec![march(army, 30)]);
    assert_eq!(outcome(&report, FIRST), no_route(30));
    // At war with nation 1, but nation 2 is still neutral.
    act(&mut simulation, FIRST, vec![war(SECOND)]);
    let report = act(&mut simulation, FIRST, vec![march(army, 40)]);
    assert_eq!(outcome(&report, FIRST), no_route(40));
    // Own and empty provinces are open; marching to where it stands is no march.
    let report = act(&mut simulation, FIRST, vec![march(army, 10)]);
    assert!(matches!(
        outcome(&report, FIRST),
        NationActionOutcome::Applied {
            effect: NationActionEffect::ArmyMarchOrdered { ref route, .. }
        } if route.is_empty()
    ));
    assert!(
        simulation
            .world()
            .army(army)
            .unwrap()
            .destination()
            .is_none()
    );
    let _ = THIRD;
}

#[test]
fn a_march_follows_the_allowed_route_and_ends_on_entering_enemy_land() {
    let mut simulation = simulation(1_000);
    let army = raise(&mut simulation, "levy", 2);
    act(&mut simulation, FIRST, vec![war(SECOND)]);
    let report = act(&mut simulation, FIRST, vec![march(army, 30)]);
    assert!(matches!(
        outcome(&report, FIRST),
        NationActionOutcome::Applied {
            effect: NationActionEffect::ArmyMarchOrdered { ref route, .. }
        } if route == &vec![20, 50, 30]
    ));
    // Speed 1: one province per turn, starting in the turn of the order.
    let marches = &report.nations[&FIRST].military.marches;
    assert_eq!(
        (marches[0].path.clone(), marches[0].outcome),
        (vec![20], MarchOutcome::Marching)
    );
    let report = simulation.step().unwrap();
    assert_eq!(
        report.nations[&FIRST].military.marches[0].path,
        vec![50],
        "a province without a settlement is open"
    );
    let report = simulation.step().unwrap();
    let marches = &report.nations[&FIRST].military.marches;
    assert_eq!(
        (marches[0].path.clone(), marches[0].outcome),
        (vec![30], MarchOutcome::Attacking)
    );
    assert_eq!(province_of(&simulation, army), Some(30));
    assert!(
        simulation
            .world()
            .army(army)
            .unwrap()
            .destination()
            .is_none()
    );
}

#[test]
fn cavalry_marches_two_provinces_and_a_neutral_nation_blocks_the_way() {
    let mut world = world(1_000, false);
    for (resource, amount) in [("cattle", 25), ("steel", 25)] {
        world
            .deposit_nation_resource(FIRST, material(resource), amount)
            .unwrap();
    }
    let mut simulation = Simulation::new(world).unwrap();
    let army = raise(&mut simulation, "cavalry", 1);
    let report = act(&mut simulation, FIRST, vec![march(army, 50)]);
    let marches = &report.nations[&FIRST].military.marches;
    assert_eq!(
        (marches[0].path.clone(), marches[0].outcome),
        (vec![20, 50], MarchOutcome::Arrived)
    );
    // 40 lies behind neutral nation 1's province 30.
    let report = act(&mut simulation, FIRST, vec![march(army, 40)]);
    assert_eq!(
        outcome(&report, FIRST),
        NationActionOutcome::Rejected {
            error: NationActionError::NoRoute {
                army_id: army,
                province_id: 40
            }
        }
    );
    let report = act(&mut simulation, FIRST, vec![march(army, 99)]);
    assert_eq!(
        outcome(&report, FIRST),
        NationActionOutcome::Rejected {
            error: NationActionError::UnknownProvince { province_id: 99 }
        }
    );
}

// ---------------------------------------------------------------- sieges and battles

/// Nation 0 declares war on nation 1 and its army marches into province 30;
/// returns the report of the turn it arrived and attacked.
fn besiege(simulation: &mut Simulation, unit: &str, units: u32) -> (ArmyId, SimulationTurnReport) {
    let army = raise(simulation, unit, units);
    act(simulation, FIRST, vec![war(SECOND), march(army, 30)]);
    simulation.step().unwrap();
    let report = simulation.step().unwrap();
    (army, report)
}

#[test]
fn a_settlement_with_an_enemy_army_is_under_attack_and_does_not_trade() {
    let mut simulation = simulation(1_000);
    let (army, report) = besiege(&mut simulation, "levy", 4);
    assert_eq!(province_of(&simulation, army), Some(30));
    assert!(simulation.world().is_under_attack(C));
    assert!(simulation.observe_nation(SECOND).unwrap().settlements[0].under_attack);
    assert_eq!(
        battles(&report, SECOND).len(),
        1,
        "the attack starts at once"
    );
    // C cannot send a caravan; the sieged settlement is also no partner for D.
    simulation
        .queue_decision(SettlementDecision::new(
            C,
            vec![SettlementAction::OrderCaravan {
                receiver_id: D,
                offers: vec![OfferSpec {
                    resource: material("grain"),
                    side: OfferSide::Buy,
                    units: 1,
                    price_per_unit: 1,
                }],
            }],
        ))
        .unwrap();
    let report = simulation.step().unwrap();
    let decision = report
        .decisions
        .iter()
        .find(|d| d.settlement_id == C)
        .unwrap();
    assert_eq!(
        decision.results[0].outcome,
        ActionOutcome::Rejected {
            error: ActionError::SettlementUnderAttack { settlement_id: C }
        }
    );
    let partners = simulation.observe(D).unwrap().trade_partners;
    assert!(
        !partners
            .iter()
            .find(|p| p.settlement_id == C)
            .unwrap()
            .trade_allowed
    );
}

#[test]
fn a_beaten_attacker_retreats_where_it_came_from_and_the_garrison_holds() {
    // One levy (attack 1) against a garrison of 50 levy (defence 1): a tie,
    // which the defenders win.
    let mut simulation = simulation(1_000);
    assert_eq!(
        simulation.observe_nation(SECOND).unwrap().settlements[0].garrison,
        50
    );
    let (army, report) = besiege(&mut simulation, "levy", 1);
    let battle = &battles(&report, FIRST)[0];
    assert_eq!(
        (battle.attacker_strength, battle.defender_strength),
        (1.0, 1.0)
    );
    assert_eq!(battle.winner, BattleSide::Defenders);
    // The loser loses 30% (15) and 30 morale; the winner 30% × 1/1 (15).
    assert_eq!((battle.attacker_losses, battle.defender_losses), (15, 15));
    assert_eq!(battle.retreated, vec![army]);
    let unit = &simulation.world().army(army).unwrap().units()[0];
    assert_eq!((unit.soldiers, unit.morale), (35, 70));
    assert_eq!(province_of(&simulation, army), Some(50));
    // The fallen of the garrison left the population; the siege is over and
    // the garrison refilled to 5% of the people left.
    assert_eq!(simulation.world().settlement(C).unwrap().population(), 985);
    assert!(!simulation.world().is_under_attack(C));
    assert_eq!(simulation.world().garrison(C).unwrap().soldiers, 49);
}

#[test]
fn a_siege_goes_on_until_the_defenders_break_and_the_settlement_falls() {
    let mut simulation = simulation(1_000);
    let (army, report) = besiege(&mut simulation, "levy", 4);
    let battle = &battles(&report, SECOND)[0];
    assert_eq!(battle.winner, BattleSide::Attackers);
    assert_eq!(
        battle.conquered_by, None,
        "a beaten defender stays: the battle goes on"
    );
    let garrison = simulation.world().garrison(C).unwrap();
    assert_eq!((garrison.soldiers, garrison.morale), (35, 70));
    assert!(
        simulation.world().is_under_attack(C),
        "the garrison does not refill under attack"
    );
    // Morale 70 → 40 → 10 → 0: the fourth battle breaks the garrison.
    let mut fallen = None;
    for _ in 0..3 {
        let report = simulation.step().unwrap();
        if let Some(battle) = battles(&report, FIRST).first() {
            fallen = fallen.or(battle.conquered_by);
        }
    }
    assert_eq!(fallen, Some(FIRST));
    assert_eq!(simulation.world().settlement_owner(C), Some(FIRST));
    // Nation 1 lost its last settlement and left the game.
    assert!(simulation.world().nation(SECOND).is_none());
    let ownership = simulation.world().nations().ownership(C).unwrap();
    assert_eq!(
        ownership.conquest_turns(),
        3,
        "half tax from the conquest on"
    );
    assert!(!simulation.world().is_under_attack(C));
    assert_eq!(province_of(&simulation, army), Some(30));
}

#[test]
fn a_settlement_without_defenders_falls_without_a_battle() {
    // 10 people: 5% is no garrison at all.
    let mut simulation = simulation(10);
    let (_, report) = besiege(&mut simulation, "levy", 1);
    let battle = &battles(&report, FIRST)[0];
    assert_eq!(battle.garrison_soldiers, 0);
    assert_eq!(battle.conquered_by, Some(FIRST));
    assert_eq!(simulation.world().settlement_owner(C), Some(FIRST));
}

/// Nation 1 has National Fortification active (+10 wall points); nation 0
/// can build a siege engine.
fn fortified() -> Simulation {
    let mut world = world(1_000, false);
    world.deposit_nation_gold(SECOND, 1_000).unwrap();
    for (resource, amount) in [("planks", 50), ("tools", 25)] {
        world
            .deposit_nation_resource(FIRST, material(resource), amount)
            .unwrap();
    }
    let mut simulation = Simulation::new(world).unwrap();
    act(
        &mut simulation,
        SECOND,
        vec![NationAction::EnactDecree {
            decree: DecreeId::new("national_fortification").unwrap(),
        }],
    );
    simulation
}

#[test]
fn walls_from_a_decree_strengthen_the_defence_and_siege_engines_remove_them() {
    let mut simulation = fortified();
    let (_, report) = besiege(&mut simulation, "levy", 1);
    let battle = &battles(&report, FIRST)[0];
    assert!((battle.wall_multiplier - 1.1).abs() < 1e-6, "+10 points");
    assert!(battle.defender_strength > battle.attacker_strength);

    // A siege engine removes 25 points: nothing of the 10 is left.
    let mut simulation = fortified();
    let (_, report) = besiege(&mut simulation, "siege_engines", 1);
    assert_eq!(battles(&report, FIRST)[0].wall_multiplier, 1.0);
}

#[test]
fn battles_follow_the_seed() {
    let run = |seed: u64| {
        let mut world = world(1_000, true);
        world.set_battle_seed(seed);
        let mut simulation = Simulation::new(world).unwrap();
        let (_, report) = besiege(&mut simulation, "levy", 2);
        battles(&report, FIRST)[0].clone()
    };
    assert_eq!(run(7), run(7), "the same seed, the same battle");
    let battle = run(7);
    // ±10% around 2.0 and 1.0.
    assert!((1.8..=2.2).contains(&battle.attacker_strength));
    assert!((0.9..=1.1).contains(&battle.defender_strength));
    assert_ne!(run(7).attacker_strength, run(8).attacker_strength);
}

// ---------------------------------------------------------------- peace

#[test]
fn after_peace_armies_on_the_other_sides_land_go_home() {
    let mut simulation = simulation(1_000);
    let (army, _) = besiege(&mut simulation, "levy", 4);
    act(
        &mut simulation,
        FIRST,
        vec![NationAction::ProposeTreaty {
            target: SECOND,
            treaty: Treaty::Peace,
        }],
    );
    let report = act(
        &mut simulation,
        SECOND,
        vec![NationAction::RespondToProposal {
            proposer: FIRST,
            treaty: Treaty::Peace,
            accept: true,
        }],
    );
    // The peace of this turn is decided before the war phase: the army
    // returns home the same turn, to the nearest own province.
    let withdrawals = &report.nations[&FIRST].military.withdrawals;
    assert_eq!(withdrawals.len(), 1);
    assert_eq!((withdrawals[0].from, withdrawals[0].to), (30, Some(20)));
    assert_eq!(province_of(&simulation, army), Some(20));
    assert!(!simulation.world().is_under_attack(C));
    assert_eq!(
        simulation.world().settlement_owner(C),
        Some(SECOND),
        "status quo"
    );
}

#[test]
fn a_nation_sees_where_its_armies_march_and_its_garrisons() {
    let mut simulation = simulation(1_000);
    let army = raise(&mut simulation, "levy", 2);
    act(&mut simulation, FIRST, vec![war(SECOND), march(army, 30)]);
    let observation = simulation.observe_nation(FIRST).unwrap();
    assert_eq!(observation.military.armies[0].destination, Some(30));
    assert_eq!(
        observation.settlements[0].garrison, 245,
        "5% of the 4900 people left after the recruitment"
    );
}
