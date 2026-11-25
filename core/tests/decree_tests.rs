//! Decrees (gameplay specification §7): the catalogue loaded from
//! `decrees.toml`, the upkeep paid every active turn, automatic revocation when
//! unpaid, delayed start, minimum duration, cooldown, slots and effects as data
//! for the systems that apply them.
mod common;
use common::material;
use simulation_core::economy::ResourceCatalog;
use simulation_core::ids::{NationId, SettlementId};
use simulation_core::nation::decree::{DecreeEffects, DecreeId, DecreeState};
use simulation_core::nation::decree_catalog::{DecreeCatalog, DecreeCatalogError};
use simulation_core::settlement::Settlement;
use simulation_core::simulation::{
    NationAction, NationActionEffect, NationActionError, NationActionOutcome, NationDecision,
    NationObservation, Simulation, SimulationTurnReport,
};
use simulation_core::world::{Map, Province, World, WorldError, terrain::area::Area};
use std::collections::BTreeMap;
use std::sync::Arc;

const A: SettlementId = SettlementId(1);
const B: SettlementId = SettlementId(2);
const C: SettlementId = SettlementId(3);
const FIRST: NationId = NationId(0);

const DECREE_TOML: &str = include_str!("../../decrees.toml");

fn id(id: &str) -> DecreeId {
    DecreeId::new(id).unwrap()
}

/// Provinces 10, 20, 30, 40 in a row. A founds nation 0 and B joins it; C
/// founds nation 1; province 40 is empty. Nation 0 holds `treasury` gold.
fn world(treasury: u32) -> World {
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
    world
}

fn simulation(treasury: u32) -> Simulation {
    Simulation::new(world(treasury)).unwrap()
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

fn enact(decree: &str) -> NationAction {
    NationAction::EnactDecree { decree: id(decree) }
}

fn revoke(decree: &str) -> NationAction {
    NationAction::RevokeDecree { decree: id(decree) }
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

fn treasury(simulation: &Simulation) -> u64 {
    simulation.world().nation(FIRST).unwrap().treasury()
}

fn state(simulation: &Simulation, decree: &str) -> Option<DecreeState> {
    simulation
        .world()
        .nation(FIRST)
        .unwrap()
        .decrees()
        .state(&id(decree))
}

/// Production change of one material under the effects.
fn production(effects: &DecreeEffects, name: &str) -> f32 {
    let catalog = ResourceCatalog::standard();
    effects.production_change(catalog.get(material(name)).unwrap())
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-6
}

// ---------------------------------------------------------------- catalogue

#[test]
fn the_bundled_catalogue_loads_from_disk_with_the_specification_table() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../decrees.toml");
    let catalog = DecreeCatalog::load(path).unwrap();
    assert_eq!(&catalog, DecreeCatalog::standard());
    let multipliers: Vec<_> = catalog
        .definitions()
        .iter()
        .map(|definition| (definition.id().as_str(), definition.cost_multiplier()))
        .collect();
    assert_eq!(
        multipliers,
        vec![
            ("agricultural_decree", 1),
            ("craft_reform", 1),
            ("state_mining", 1),
            ("great_construction", 2),
            ("free_trade", 1),
            ("protectionism", 1),
            ("military_levy", 1),
            ("mobilization", 1),
            ("discipline", 1),
            ("food_rationing", 1),
            ("games", 4),
            ("art_patronage", 3),
            ("national_fortification", 4),
        ]
    );
    let rules = catalog.rules();
    assert_eq!(
        (
            rules.base_upkeep_per_settlement,
            rules.minimum_active_turns,
            rules.cooldown_turns
        ),
        (10, 5, 5)
    );
    assert_eq!(catalog.upkeep(&id("games"), 3), Some(120));
    assert_eq!(catalog.upkeep(&id("great_construction"), 3), Some(60));
    assert_eq!(catalog.upkeep(&id("free_beer"), 3), None);
}

#[test]
fn slots_grow_with_average_happiness_from_seventy() {
    let rules = DecreeCatalog::standard().rules();
    for (happiness, slots) in [
        (None, 2),
        (Some(69.9), 2),
        (Some(70.0), 3),
        (Some(79.9), 3),
        (Some(80.0), 4),
        (Some(90.0), 5),
        (Some(100.0), 6),
        (Some(250.0), 6),
    ] {
        assert_eq!(rules.slots(happiness), slots, "{happiness:?}");
    }
}

#[test]
fn effects_from_the_file_stack_across_decrees() {
    let catalog = DecreeCatalog::standard();
    let effects = catalog.effects_of(&[
        id("agricultural_decree"),
        id("craft_reform"),
        id("mobilization"),
        id("free_trade"),
        id("protectionism"),
        id("games"),
        id("food_rationing"),
    ]);
    // Grain: +0.2 (raw food tier 1) −0.1 (tier 1) −0.15 (everything).
    assert!(close(production(&effects, "grain"), -0.05));
    // Wood: −0.1 (building material tier 1) −0.1 (tier 1) −0.15.
    assert!(close(production(&effects, "wood"), -0.35));
    // Bread (tier 2): +0.2 −0.15.
    assert!(close(production(&effects, "bread"), 0.05));
    assert_eq!(effects.tariff_multiplier, 1.0, "×0.5 × ×2");
    assert_eq!(effects.caravan_speed, 0.25);
    assert_eq!(effects.happiness, 15.0 - 10.0 - 15.0);
    assert_eq!(effects.food_consumption, -0.25);
    assert!(effects.instant_recruitment);
    // State mining raises ores and coal (tier-1 metals) only.
    let mining = catalog.effects_of(&[id("state_mining")]);
    assert!(close(production(&mining, "iron_ore"), 0.3));
    assert!(close(production(&mining, "coal"), 0.3));
    assert_eq!(production(&mining, "grain"), 0.0);
    assert_eq!(catalog.effects_of(&[]), DecreeEffects::default());
}

#[test]
fn invalid_catalogues_are_rejected_when_loading() {
    let rules = "[rules]\nbase_upkeep_per_settlement = 10\nminimum_active_turns = 5\n\
        cooldown_turns = 5\nbase_slots = 2\nmax_slots = 6\nextra_slot_happiness = 70.0\n\
        extra_slot_happiness_step = 10.0\n";
    let with = |decrees: &str| DecreeCatalog::from_toml(&format!("{rules}{decrees}"));
    let decree = |id: &str, extra: &str| {
        format!("[[decrees]]\nid = \"{id}\"\nname = \"Name\"\ncost_multiplier = 1\n{extra}\n")
    };
    assert!(with("").unwrap().definitions().is_empty());
    assert!(matches!(
        with(&decree("Games", "")),
        Err(DecreeCatalogError::InvalidIdentifier(id)) if id == "Games"
    ));
    assert!(matches!(
        with(&format!("{}{}", decree("games", ""), decree("games", ""))),
        Err(DecreeCatalogError::DuplicateDecree(_))
    ));
    assert!(matches!(
        with(&decree("games", "").replace("Name", " ")),
        Err(DecreeCatalogError::EmptyName(_))
    ));
    assert!(matches!(
        with(&decree("games", "").replace("cost_multiplier = 1", "cost_multiplier = 0")),
        Err(DecreeCatalogError::ZeroCostMultiplier(_))
    ));
    assert!(matches!(
        with(&decree(
            "games",
            "[decrees.effects]\nproduction = [{ tiers = [4], change = 0.1 }]"
        )),
        Err(DecreeCatalogError::InvalidProductionTier { tier: 4, .. })
    ));
    assert!(matches!(
        with(&decree("games", "[decrees.effects]\nhappiness = nan")),
        Err(DecreeCatalogError::NonFiniteEffect(_))
    ));
    for unknown in [
        "[decrees.effects]\nfree_beer = 1.0",
        "[decrees.effects]\nproduction = [{ categories = [\"gold\"], change = 0.1 }]",
        "upkeep = 3",
    ] {
        assert!(
            matches!(
                with(&decree("games", unknown)),
                Err(DecreeCatalogError::Parse(_))
            ),
            "{unknown}"
        );
    }
    assert!(matches!(
        DecreeCatalog::from_toml(&rules.replace("base_slots = 2", "base_slots = 7")),
        Err(DecreeCatalogError::InvalidRules(_))
    ));
    assert!(matches!(
        DecreeCatalog::from_toml(&rules.replace("step = 10.0", "step = 0.0")),
        Err(DecreeCatalogError::InvalidRules(_))
    ));
    assert!(DecreeCatalog::from_toml(&DECREE_TOML.replace("[rules]", "[rule]")).is_err());
}

#[test]
fn a_world_can_use_its_own_catalogue_until_a_decree_is_enacted() {
    let source = DECREE_TOML
        .replace(
            "base_upkeep_per_settlement = 10",
            "base_upkeep_per_settlement = 7",
        )
        .replace("minimum_active_turns = 5", "minimum_active_turns = 1")
        .replace("id = \"games\"", "id = \"festival\"");
    let mut world = world(1_000);
    world
        .set_decree_catalog(Arc::new(DecreeCatalog::from_toml(&source).unwrap()))
        .unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    let report = act(&mut simulation, vec![enact("games"), enact("festival")]);
    let results = outcomes(&report);
    assert_eq!(
        error(&results[0]),
        NationActionError::UnknownDecree {
            decree: id("games")
        }
    );
    assert_eq!(
        results[1],
        NationActionOutcome::Applied {
            // 7 gold × 2 settlements × 4.
            effect: NationActionEffect::DecreeEnacted {
                decree: id("festival"),
                upkeep: 56
            }
        }
    );
    assert_eq!(simulation.world().decree_effects(A).happiness, 15.0);
    // One active turn is enough with this catalogue's rules.
    simulation.step().unwrap();
    let report = act(&mut simulation, vec![revoke("festival")]);
    assert!(applied(&outcomes(&report)[0]));

    let mut world = simulation.into_world();
    assert_eq!(
        world.set_decree_catalog(Arc::new(DecreeCatalog::standard().clone())),
        Err(WorldError::DecreeCatalogInUse),
        "the revoked decree is still cooling down"
    );
}

// ---------------------------------------------------------------- upkeep and timing

#[test]
fn a_decree_works_from_the_next_turn_and_costs_its_upkeep_every_active_turn() {
    let mut simulation = simulation(1_000);
    queue(&mut simulation, vec![enact("agricultural_decree")]);
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
                decree: id("agricultural_decree"),
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
            id("agricultural_decree"),
            DecreeState::Active { turns_active: 0 }
        )])
    );
    let effects = simulation.world().decree_effects(B);
    assert!(close(production(&effects, "grain"), 0.2));
    assert!(close(production(&effects, "stone"), -0.1));
    // Turns 2..=6: 20 gold every turn.
    for turn in 1..=5 {
        let report = simulation.step().unwrap();
        let upkeep = &report.nations[&FIRST].decree_upkeep;
        assert_eq!(upkeep.paid, vec![id("agricultural_decree")]);
        assert_eq!(upkeep.gold_paid, 20);
        assert_eq!(treasury(&simulation), 1_000 - 20 * turn);
    }
}

#[test]
fn a_decree_the_treasury_cannot_pay_is_revoked_like_any_other() {
    // Games cost 10 × 2 × 4 = 80 per turn; the treasury holds 100.
    let mut simulation = simulation(100);
    let report = act(&mut simulation, vec![enact("games")]);
    assert!(applied(&outcomes(&report)[0]), "enacting needs no gold");
    let report = simulation.step().unwrap();
    assert_eq!(report.nations[&FIRST].decree_upkeep.gold_paid, 80);
    assert_eq!(treasury(&simulation), 20);
    let report = simulation.step().unwrap();
    let upkeep = &report.nations[&FIRST].decree_upkeep;
    assert_eq!(upkeep.revoked_unpaid, vec![id("games")]);
    assert!(upkeep.paid.is_empty());
    assert_eq!(treasury(&simulation), 20, "an unpaid decree takes nothing");
    assert_eq!(
        state(&simulation, "games"),
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
    act(&mut simulation, vec![enact("great_construction")]);
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
    act(&mut simulation, vec![enact("games")]);
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
    act(&mut simulation, vec![enact("games")]);
    for turns_left in (1..=5).rev() {
        let report = act(&mut simulation, vec![revoke("games")]);
        assert_eq!(
            error(&outcomes(&report)[0]),
            NationActionError::DecreeMinimumDuration {
                decree: id("games"),
                turns_left
            }
        );
    }
    let report = act(&mut simulation, vec![revoke("games")]);
    assert_eq!(
        outcomes(&report)[0],
        NationActionOutcome::Applied {
            effect: NationActionEffect::DecreeRevoked {
                decree: id("games"),
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
    act(&mut simulation, vec![enact("free_trade")]);
    for _ in 0..5 {
        simulation.step().unwrap();
    }
    // Revoke, and the same decree cannot come back in this decision.
    let report = act(
        &mut simulation,
        vec![revoke("free_trade"), enact("free_trade")],
    );
    let results = outcomes(&report);
    assert!(applied(&results[0]));
    assert_eq!(
        error(&results[1]),
        NationActionError::Conflict { with_index: 0 }
    );
    for turns_left in (1..=4).rev() {
        let report = act(&mut simulation, vec![enact("free_trade")]);
        assert_eq!(
            error(&outcomes(&report)[0]),
            NationActionError::DecreeOnCooldown {
                decree: id("free_trade"),
                turns_left
            }
        );
    }
    assert_eq!(state(&simulation, "free_trade"), None);
    let report = act(&mut simulation, vec![enact("free_trade")]);
    assert!(applied(&outcomes(&report)[0]));
    // Five paid active turns before the revocation; the new one is pending.
    assert_eq!(treasury(&simulation), 1_000 - 5 * 20);
}

#[test]
fn unknown_decrees_are_rejected_by_validation_not_by_parsing() {
    let mut simulation = simulation(1_000);
    let decision = NationDecision::from_json(
        r#"{"nation_id": 0, "actions": [
            {"type": "enact_decree", "decree": "free_beer"},
            {"type": "revoke_decree", "decree": "free_beer"}
        ]}"#,
    )
    .unwrap();
    simulation.queue_nation_decision(decision).unwrap();
    let report = simulation.step().unwrap();
    for outcome in outcomes(&report) {
        assert_eq!(
            error(&outcome),
            NationActionError::UnknownDecree {
                decree: id("free_beer")
            }
        );
    }
    assert!(
        simulation
            .world()
            .nation(FIRST)
            .unwrap()
            .decrees()
            .states()
            .is_empty()
    );
}

// ---------------------------------------------------------------- slots

#[test]
fn pending_and_active_decrees_use_the_two_slots_and_a_revocation_frees_one() {
    let mut simulation = simulation(10_000);
    // Turn 1: two decrees fit; a third does not, and a repeat conflicts.
    queue(
        &mut simulation,
        vec![
            enact("games"),
            enact("games"),
            enact("discipline"),
            enact("protectionism"),
        ],
    );
    // A second decision of the same turn sees the pending decrees.
    queue(
        &mut simulation,
        vec![revoke("discipline"), enact("discipline")],
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
            decree: id("discipline")
        },
        "a pending decree cannot be revoked"
    );
    assert_eq!(
        error(&second[1]),
        NationActionError::DecreeAlreadyEnacted {
            decree: id("discipline")
        }
    );
    for _ in 0..5 {
        simulation.step().unwrap();
    }
    // Revoking frees a slot for another decree in the same decision.
    let report = act(
        &mut simulation,
        vec![revoke("games"), enact("protectionism")],
    );
    assert!(outcomes(&report).iter().all(applied));
    let decrees = simulation.observe_nation(FIRST).unwrap().decrees;
    assert_eq!((decrees.slots, decrees.used_slots), (2, 2));
}

// ---------------------------------------------------------------- observation

#[test]
fn nations_and_settlements_observe_decrees() {
    let mut simulation = simulation(1_000);
    act(&mut simulation, vec![enact("agricultural_decree")]);
    let observation = simulation.observe_nation(FIRST).unwrap();
    let decrees = &observation.decrees;
    assert_eq!((decrees.slots, decrees.used_slots), (2, 1));
    assert_eq!(
        (decrees.minimum_active_turns, decrees.cooldown_turns),
        (5, 5)
    );
    assert_eq!(
        decrees.states,
        BTreeMap::from([(
            id("agricultural_decree"),
            DecreeState::Active { turns_active: 0 }
        )])
    );
    assert_eq!(
        decrees.upkeep.len(),
        DecreeCatalog::standard().definitions().len()
    );
    assert_eq!(decrees.upkeep[&id("games")], 80);
    assert!(close(production(&decrees.effects, "grain"), 0.2));
    assert_eq!(
        serde_json::from_str::<NationObservation>(&observation.to_json()).unwrap(),
        observation
    );
    assert_eq!(
        simulation.observe(A).unwrap().nation.active_decrees,
        vec![id("agricultural_decree")]
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
                    {"type": "enact_decree", "decree": "great_construction"}
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
    let report = simulation.step().unwrap();
    let nation = serde_json::to_value(&report.nations[&FIRST]).unwrap();
    assert_eq!(
        nation["decrees"],
        serde_json::json!({"great_construction": {"state": "active", "turns_active": 1}})
    );
    assert_eq!(
        nation["decree_upkeep"],
        serde_json::json!({"paid": ["great_construction"], "gold_paid": 40, "revoked_unpaid": []})
    );
}
