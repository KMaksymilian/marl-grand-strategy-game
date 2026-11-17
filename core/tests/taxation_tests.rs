//! Taxation (gameplay specification §6): the owning nation takes its rate from
//! every settlement's production and sums it in the national stock. Covers rates
//! 0, 10 and 100%, per-stage production tax, rounding, conquered and attacked
//! settlements, national capacity overflow, balances and the nation's report.
//! Gold is not taxed yet: the gold tax needs settlement income (§4.4), which
//! does not exist in the economy yet.
use simulation_core::buildings::Building;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::economy::ResourceType::{self, Stone, Wood};
use simulation_core::ids::{BuildingId, NationId, SettlementId};
use simulation_core::nation::TaxRate;
use simulation_core::nation::taxation::TaxStatus;
use simulation_core::settlement::Settlement;
use simulation_core::settlement::projects::ProjectCost;
use simulation_core::simulation::{
    NationAction, NationDecision, NationObservation, SettlementAction, SettlementDecision,
    Simulation, SimulationTurnReport,
};
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::{Map, Position, Province, World, terrain::area::Area};
use std::collections::BTreeMap;
use std::sync::Arc;

const A: SettlementId = SettlementId(1);
const B: SettlementId = SettlementId(2);
const C: SettlementId = SettlementId(3);
const FIRST: NationId = NationId(0);
const SECOND: NationId = NationId(1);

fn at(x: i32) -> Position {
    Position { x, y: 0 }
}

fn definition(
    id: &str,
    inputs: Vec<(ResourceType, u32)>,
    outputs: Vec<(ResourceType, u32)>,
) -> Arc<BuildingDefinition> {
    Arc::new(
        BuildingDefinition::new(
            id,
            id,
            1,
            10,
            vec![BuildingBehavior::Production { inputs, outputs }],
            ProjectCost::default(),
        )
        .unwrap(),
    )
}

/// Provinces `10 10 20 20 30 30`. A founds nation 0 and B joins it; C founds
/// nation 1. Each owns a camp producing 100 Wood per turn next to its capital.
/// Warehouses hold 10 000; A starts with 200 Wood and 7 gold.
fn world() -> World {
    let mut world = World::new(
        Map::new(
            6,
            1,
            vec![Area::new(0.5, 0.5); 6],
            vec![10, 10, 20, 20, 30, 30],
        ),
        [10, 20, 30].into_iter().map(Province::new).collect(),
    );
    let mut a = Settlement::new(A, 10_000, 100, Arc::default());
    a.deposit(Wood, 200);
    a.deposit_gold(7);
    world.add_settlement_at(10, at(0), a).unwrap();
    world
        .add_settlement_to_nation(FIRST, 20, Settlement::new(B, 10_000, 50, Arc::default()))
        .unwrap();
    world
        .add_settlement_at(30, at(4), Settlement::new(C, 10_000, 40, Arc::default()))
        .unwrap();
    let camp = definition("camp", vec![], vec![(Wood, 100)]);
    let mut finder = PathFinder::new(6, 1);
    for (settlement, x) in [(A, 1), (B, 3), (C, 5)] {
        world
            .add_building(
                settlement,
                Building::new(BuildingId(1), at(x), camp.clone()),
                &mut finder,
            )
            .unwrap();
    }
    world
}

/// A also gets a mason turning 50 Wood into 25 Stone: a second production stage.
fn with_mason(mut world: World) -> World {
    let mason = definition("mason", vec![(Wood, 50)], vec![(Stone, 25)]);
    world
        .add_building(
            A,
            Building::new(BuildingId(2), at(0), mason),
            &mut PathFinder::new(6, 1),
        )
        .unwrap();
    world
}

/// Staffs every building fully in the first turn, so each turn produces the
/// same amounts.
fn simulation(world: World) -> Simulation {
    let buildings: Vec<_> = world
        .settlements()
        .map(|settlement| {
            (
                settlement.id(),
                settlement
                    .buildings()
                    .map(|building| building.instance_id())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let mut simulation = Simulation::new(world).unwrap();
    for (settlement, ids) in buildings {
        let actions = ids
            .into_iter()
            .map(|building_id| SettlementAction::SetBuildingWorkers {
                building_id,
                workers: 10,
            })
            .collect();
        simulation
            .queue_decision(SettlementDecision::new(settlement, actions))
            .unwrap();
    }
    simulation
}

fn set_rate(simulation: &mut Simulation, nation: NationId, fraction: f64) {
    simulation
        .queue_nation_decision(NationDecision::new(
            nation,
            vec![NationAction::SetTaxRate {
                rate: TaxRate::from_fraction(fraction).unwrap(),
            }],
        ))
        .unwrap();
}

fn assert_balanced(report: &SimulationTurnReport) {
    for (id, settlement) in &report.settlements {
        assert!(settlement.inventory.is_balanced(), "{id:?}");
    }
}

fn wood_tax(report: &SimulationTurnReport, settlement: SettlementId) -> u64 {
    report.settlements[&settlement]
        .taxation
        .as_ref()
        .unwrap()
        .production_tax
        .get(&Wood)
        .copied()
        .unwrap_or(0)
}

// ---------------------------------------------------------------- rates

#[test]
fn the_base_rate_takes_ten_percent_of_production() {
    let mut simulation = simulation(world());
    let report = simulation.step().unwrap();
    assert_balanced(&report);
    let b = report.settlements[&B].taxation.as_ref().unwrap();
    assert_eq!(b.nation_id, FIRST);
    assert_eq!(b.status, TaxStatus::Regular);
    assert_eq!(
        (b.nominal_rate, b.effective_rate),
        (TaxRate::BASE, TaxRate::BASE)
    );
    assert_eq!(b.production_tax, BTreeMap::from([(Wood, 10)]));
    assert_eq!(
        report.settlements[&B].inventory.produced,
        BTreeMap::from([(Wood, 100)]),
        "the report keeps gross production"
    );
    assert_eq!(report.settlements[&B].inventory.taxed, b.production_tax);
    assert_eq!(
        simulation.settlement(B).unwrap().inventory().amount(Wood),
        90
    );
}

#[test]
fn rates_zero_ten_and_one_hundred_percent() {
    for (fraction, per_camp) in [(0.0, 0), (0.1, 10), (1.0, 100)] {
        let mut simulation = simulation(world());
        set_rate(&mut simulation, FIRST, fraction);
        let report = simulation.step().unwrap();
        assert_balanced(&report);
        assert_eq!(wood_tax(&report, A), per_camp, "{fraction}");
        assert_eq!(wood_tax(&report, B), per_camp, "{fraction}");
        // The nation's sum is what all its settlements paid.
        let nation = &report.nations[&FIRST];
        assert_eq!(nation.tax_rate, TaxRate::from_fraction(fraction).unwrap());
        assert_eq!(
            nation.stock_received.get(&Wood).copied().unwrap_or(0),
            2 * per_camp,
            "{fraction}"
        );
        assert_eq!(
            u64::from(
                simulation
                    .world()
                    .nation(FIRST)
                    .unwrap()
                    .stock()
                    .amount(Wood)
            ),
            2 * per_camp
        );
        // The other nation keeps its own base rate.
        assert_eq!(wood_tax(&report, C), 10);
    }
}

#[test]
fn at_one_hundred_percent_a_settlement_keeps_none_of_its_production() {
    let mut simulation = simulation(world());
    set_rate(&mut simulation, FIRST, 1.0);
    simulation.step().unwrap();
    assert_eq!(
        simulation.settlement(B).unwrap().inventory().amount(Wood),
        0
    );
}

#[test]
fn taxation_does_not_touch_gold() {
    let mut simulation = simulation(world());
    set_rate(&mut simulation, FIRST, 1.0);
    for _ in 0..3 {
        simulation.step().unwrap();
    }
    assert_eq!(simulation.settlement(A).unwrap().gold(), 7);
    assert_eq!(simulation.world().nation(FIRST).unwrap().treasury(), 0);
}

// ---------------------------------------------------------------- stages and rounding

#[test]
fn every_production_stage_is_taxed_separately() {
    let mut simulation = simulation(with_mason(world()));
    let report = simulation.step().unwrap();
    assert_balanced(&report);
    let a = &report.settlements[&A];
    // The camp produced 100 Wood; the mason turned 50 Wood from stock into 25 Stone.
    assert_eq!(
        a.inventory.produced,
        BTreeMap::from([(Wood, 100), (Stone, 25)])
    );
    assert_eq!(
        a.taxation.as_ref().unwrap().production_tax,
        BTreeMap::from([(Wood, 10), (Stone, 2)]),
        "2.5 Stone rounds down"
    );
    // 200 − 50 consumed + 100 produced − 10 taxed.
    assert_eq!(
        simulation.settlement(A).unwrap().inventory().amount(Wood),
        240
    );
    assert_eq!(
        simulation.settlement(A).unwrap().inventory().amount(Stone),
        23
    );
}

#[test]
fn taxes_round_down_to_whole_units() {
    // 29% of 100 is 29 even though 0.29 × 100 is 28.999… in floating point;
    // 37% of 25 Stone is 9.25, so 9.
    let mut first = simulation(with_mason(world()));
    set_rate(&mut first, FIRST, 0.29);
    let report = first.step().unwrap();
    assert_eq!(wood_tax(&report, A), 29);

    let mut second = simulation(with_mason(world()));
    set_rate(&mut second, FIRST, 0.37);
    let report = second.step().unwrap();
    let a = report.settlements[&A].taxation.as_ref().unwrap();
    assert_eq!(a.production_tax, BTreeMap::from([(Wood, 37), (Stone, 9)]));
}

// ---------------------------------------------------------------- status

#[test]
fn a_freshly_conquered_settlement_pays_half_until_the_penalty_ends() {
    let mut world = world();
    world.transfer_settlement(C, FIRST).unwrap();
    let mut simulation = simulation(world);
    // Penalty 20, 15, 10, 5: half the rate; then the full rate.
    for (turn, (status, tax)) in [
        (TaxStatus::FreshlyConquered, 5),
        (TaxStatus::FreshlyConquered, 5),
        (TaxStatus::FreshlyConquered, 5),
        (TaxStatus::FreshlyConquered, 5),
        (TaxStatus::Regular, 10),
    ]
    .into_iter()
    .enumerate()
    {
        let report = simulation.step().unwrap();
        let c = report.settlements[&C].taxation.as_ref().unwrap();
        assert_eq!(c.nation_id, FIRST, "turn {turn}");
        assert_eq!(c.status, status, "turn {turn}");
        assert_eq!(c.nominal_rate, TaxRate::BASE);
        assert_eq!(wood_tax(&report, C), tax, "turn {turn}");
    }
}

#[test]
fn a_settlement_under_attack_pays_no_tax() {
    let mut simulation = simulation(world());
    simulation.set_settlement_under_attack(B, true).unwrap();
    let report = simulation.step().unwrap();
    let b = report.settlements[&B].taxation.as_ref().unwrap();
    assert_eq!(b.status, TaxStatus::UnderAttack);
    assert_eq!(b.effective_rate, TaxRate::ZERO);
    assert!(b.production_tax.is_empty());
    assert_eq!(
        simulation.settlement(B).unwrap().inventory().amount(Wood),
        100
    );
    assert!(
        simulation
            .observe_nation(FIRST)
            .unwrap()
            .settlements
            .iter()
            .any(|settlement| settlement.settlement_id == B && settlement.under_attack)
    );
    // The siege ends: taxes resume next turn.
    simulation.set_settlement_under_attack(B, false).unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.settlements[&B].taxation.as_ref().unwrap().status,
        TaxStatus::Regular
    );
}

// ---------------------------------------------------------------- capacity and report

#[test]
fn production_tax_above_the_national_capacity_is_lost_and_reported() {
    let mut world = world();
    // Nation 0 owns two settlements: 1 000 per material. Nearly full already.
    world.deposit_nation_resource(FIRST, Wood, 950).unwrap();
    let mut simulation = simulation(world);
    set_rate(&mut simulation, FIRST, 1.0);
    let report = simulation.step().unwrap();
    let nation = &report.nations[&FIRST];
    // A pays 100 Wood first (ascending ID): 50 fit. B's 100 are lost entirely.
    assert_eq!(nation.stock_received, BTreeMap::from([(Wood, 50)]));
    assert_eq!(nation.stock_lost, BTreeMap::from([(Wood, 150)]));
    assert_eq!(nation.closing_stock, BTreeMap::from([(Wood, 1_000)]));
    let paid: u64 = nation
        .settlements
        .values()
        .map(|tax| tax.production_tax[&Wood])
        .sum();
    assert_eq!(
        paid,
        nation.stock_received[&Wood] + nation.stock_lost[&Wood]
    );
}

#[test]
fn nations_get_their_own_report_and_see_it_in_their_observation() {
    let mut simulation = simulation(world());
    let report = simulation.step().unwrap();
    assert_eq!(
        report.nations.keys().copied().collect::<Vec<_>>(),
        vec![FIRST, SECOND]
    );
    let first = &report.nations[&FIRST];
    assert_eq!(
        first.settlements.keys().copied().collect::<Vec<_>>(),
        vec![A, B]
    );
    assert_eq!(first.stock_received, BTreeMap::from([(Wood, 20)]));
    assert!(first.stock_lost.is_empty());
    assert_eq!(first.closing_stock, BTreeMap::from([(Wood, 20)]));
    assert_eq!(
        report.nations[&SECOND]
            .settlements
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![C]
    );

    let observation = simulation.observe_nation(FIRST).unwrap();
    assert_eq!(observation.last_turn.as_ref(), Some(first));
    assert_eq!(observation.stock, BTreeMap::from([(Wood, 20)]));
    let json = observation.to_json();
    assert_eq!(
        serde_json::from_str::<NationObservation>(&json).unwrap(),
        observation
    );

    let settlement = simulation.observe(B).unwrap().last_turn.unwrap();
    assert_eq!(settlement.production_tax, BTreeMap::from([(Wood, 10)]));
}

#[test]
fn reports_serialize_rates_as_fractions() {
    let mut simulation = simulation(world());
    set_rate(&mut simulation, FIRST, 0.25);
    let report = simulation.step().unwrap();
    let json = serde_json::to_value(&report.nations[&FIRST]).unwrap();
    assert_eq!(json["tax_rate"], serde_json::json!(0.25));
    let b = &json["settlements"]["2"];
    assert_eq!(b["status"], "regular");
    assert_eq!(b["effective_rate"], serde_json::json!(0.25));
    assert_eq!(b["production_tax"]["Wood"], serde_json::json!(25));
}
