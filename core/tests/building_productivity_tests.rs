use core::domain::geography::area as test_area;
use core::domain::world_data::map as core_map;
use core::services::algorithms::path_finder as test_path;

fn test_map() -> core_map::Map {
    core_map::Map::new(
        64,
        8,
        vec![test_area::Area::new(0.5, 0.5); 64 * 8],
        vec![0; 64 * 8],
    )
}

fn test_finder() -> test_path::PathFinder {
    test_path::PathFinder::new(64, 8)
}
use core::domain::buildings::core::{Building, Position};
use core::domain::buildings::factory::{BuildingBehavior, BuildingDefinition};
use core::domain::buildings::productivity::{
    BuildingProductivityCalculator, TravelCostEfficiencyPolicy,
};
use core::domain::economy::resource::ResourceType;
use core::domain::ids::{BuildingId, ProjectId};
use core::domain::settlement::core::Settlement;
use core::domain::settlement::projects::{ProjectCompletion, ProjectCost};
use std::sync::Arc;

fn definition(workers: u32, input: u32, output: u32) -> Arc<BuildingDefinition> {
    Arc::new(BuildingDefinition {
        id: "workshop".into(),
        name: "Workshop".into(),
        tier: 1,
        max_workers: workers,
        behaviors: vec![BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, input)],
            outputs: vec![(ResourceType::Stone, output)],
        }],
        construction_cost: ProjectCost::default(),
    })
}

fn calculator(capital_x: i32, full_steps: u32, zero_steps: u32) -> BuildingProductivityCalculator {
    BuildingProductivityCalculator::new(
        Position { x: capital_x, y: 0 },
        TravelCostEfficiencyPolicy::new(full_steps * 10, zero_steps * 10).unwrap(),
    )
}

fn settlement_at(position: Position) -> Settlement {
    let mut settlement = Settlement::new(1, 200, 10, Arc::default());
    let mut building = Building::new(BuildingId(7), position, definition(10, 10, 20)).unwrap();
    building.set_target_load(1.0).unwrap();
    settlement
        .add_building(building, &test_map(), &mut test_finder())
        .unwrap();
    settlement.deposit(ResourceType::Wood, 100);
    settlement
}

#[test]
fn distance_reduces_output_without_discounting_materials_or_workers() {
    for (x, expected_output) in [(0, 20), (10, 20), (20, 10), (30, 0), (40, 0)] {
        let mut settlement = settlement_at(Position { x, y: 0 });
        settlement.set_building_productivity_calculator(
            Some(calculator(0, 10, 30)),
            &test_map(),
            &mut test_finder(),
        );
        let report = settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(settlement.inventory().amount(ResourceType::Wood), 90);
        assert_eq!(
            settlement.inventory().amount(ResourceType::Stone),
            expected_output
        );
        assert_eq!(settlement.buildings().next().unwrap().current_workers(), 10);
        assert_eq!(
            report.inventory.production_consumed[&ResourceType::Wood],
            10
        );
        assert!(report.inventory.is_balanced());
    }
}

#[test]
fn configured_calculator_applies_to_buildings_added_later() {
    let mut settlement = Settlement::new(1, 100, 10, Arc::default());
    settlement.set_building_productivity_calculator(
        Some(calculator(0, 10, 30)),
        &test_map(),
        &mut test_finder(),
    );
    let mut building = Building::new(
        BuildingId(7),
        Position { x: 20, y: 0 },
        definition(10, 10, 20),
    )
    .unwrap();
    building.set_target_load(1.0).unwrap();
    settlement
        .add_building(building, &test_map(), &mut test_finder())
        .unwrap();
    assert_eq!(
        settlement
            .buildings()
            .next()
            .unwrap()
            .location_productivity(),
        0.5
    );
    settlement.deposit(ResourceType::Wood, 10);
    let report = settlement.process_turn(&test_map(), &mut test_finder());
    assert_eq!(settlement.inventory().amount(ResourceType::Stone), 10);
    assert!(report.inventory.is_balanced());
}

#[test]
fn moving_capital_changing_policy_and_disabling_calculator_refresh_cached_productivity() {
    let mut settlement = settlement_at(Position { x: 20, y: 0 });
    assert_eq!(settlement.building_productivity_calculator(), None);
    settlement.set_building_productivity_calculator(
        Some(calculator(0, 10, 30)),
        &test_map(),
        &mut test_finder(),
    );
    assert_eq!(
        settlement
            .buildings()
            .next()
            .unwrap()
            .location_productivity(),
        0.5
    );
    settlement.set_building_productivity_calculator(
        Some(calculator(20, 10, 30)),
        &test_map(),
        &mut test_finder(),
    );
    assert_eq!(
        settlement
            .buildings()
            .next()
            .unwrap()
            .location_productivity(),
        1.0
    );
    let changed = calculator(0, 0, 40);
    settlement.set_building_productivity_calculator(Some(changed), &test_map(), &mut test_finder());
    assert_eq!(settlement.building_productivity_calculator(), Some(changed));
    assert_eq!(
        settlement
            .buildings()
            .next()
            .unwrap()
            .location_productivity(),
        0.5
    );
    settlement.set_building_productivity_calculator(None, &test_map(), &mut test_finder());
    assert_eq!(
        settlement
            .buildings()
            .next()
            .unwrap()
            .location_productivity(),
        1.0
    );
    let report = settlement.process_turn(&test_map(), &mut test_finder());
    assert_eq!(settlement.inventory().amount(ResourceType::Stone), 20);
    assert!(report.inventory.is_balanced());
}

#[test]
fn load_and_material_shortage_remain_independent_of_location_productivity() {
    let mut settlement = settlement_at(Position { x: 20, y: 0 });
    settlement.set_building_productivity_calculator(
        Some(calculator(0, 10, 30)),
        &test_map(),
        &mut test_finder(),
    );
    settlement.set_building_load(BuildingId(7), 0.5).unwrap();
    let report = settlement.process_turn(&test_map(), &mut test_finder());
    assert_eq!(settlement.buildings().next().unwrap().current_workers(), 5);
    assert_eq!(report.inventory.production_consumed[&ResourceType::Wood], 5);
    assert_eq!(settlement.inventory().amount(ResourceType::Stone), 5);
    assert!(report.inventory.is_balanced());

    settlement.withdraw(ResourceType::Wood, 93);
    settlement.set_building_load(BuildingId(7), 1.0).unwrap();
    let report = settlement.process_turn(&test_map(), &mut test_finder());
    assert_eq!(report.inventory.production_consumed[&ResourceType::Wood], 2);
    assert_eq!(settlement.inventory().amount(ResourceType::Stone), 7);
    assert!(report.inventory.is_balanced());
}

#[test]
fn zero_load_and_inactive_buildings_do_not_consume_materials() {
    for active in [true, false] {
        let mut settlement = settlement_at(Position { x: 20, y: 0 });
        settlement.set_building_productivity_calculator(
            Some(calculator(0, 10, 30)),
            &test_map(),
            &mut test_finder(),
        );
        if active {
            settlement.set_building_load(BuildingId(7), 0.0).unwrap();
        } else {
            settlement
                .set_building_active(BuildingId(7), false)
                .unwrap();
        }
        let report = settlement.process_turn(&test_map(), &mut test_finder());
        assert_eq!(settlement.inventory().amount(ResourceType::Wood), 100);
        assert_eq!(settlement.inventory().amount(ResourceType::Stone), 0);
        assert_eq!(settlement.buildings().next().unwrap().current_workers(), 0);
        assert!(report.inventory.is_balanced());
    }
}

#[test]
fn completed_construction_and_upgrade_keep_the_spatial_penalty() {
    let mut settlement = Settlement::new(1, 100, 10, Arc::default());
    settlement.set_building_productivity_calculator(
        Some(calculator(0, 10, 30)),
        &test_map(),
        &mut test_finder(),
    );
    settlement
        .queue_construction(
            ProjectId(2),
            Position { x: 20, y: 0 },
            definition(10, 10, 20),
            ProjectCost::default(),
        )
        .unwrap();
    let report = settlement.process_turn(&test_map(), &mut test_finder());
    assert_eq!(
        report.projects[0].result,
        Ok(ProjectCompletion::BuildingConstructed(BuildingId(0)))
    );
    let building = settlement.buildings().next().unwrap();
    assert_eq!(building.location_productivity(), 0.5);
    assert_eq!(building.target_load(), 0.0);
    settlement.set_building_load(BuildingId(0), 1.0).unwrap();
    settlement.deposit(ResourceType::Wood, 20);
    settlement
        .queue_upgrade(
            ProjectId(3),
            BuildingId(0),
            definition(10, 10, 40),
            ProjectCost::default(),
        )
        .unwrap();
    let report = settlement.process_turn(&test_map(), &mut test_finder());
    assert_eq!(
        report.projects[0].result,
        Ok(ProjectCompletion::BuildingUpgraded(BuildingId(0)))
    );
    assert_eq!(settlement.inventory().amount(ResourceType::Stone), 10);
    assert!(report.inventory.is_balanced());
    assert_eq!(
        settlement
            .buildings()
            .next()
            .unwrap()
            .location_productivity(),
        0.5
    );
    let report = settlement.process_turn(&test_map(), &mut test_finder());
    assert_eq!(settlement.inventory().amount(ResourceType::Stone), 30);
    assert!(report.inventory.is_balanced());
}

#[test]
fn output_rounds_once_after_combining_staffing_and_distance() {
    let mut settlement = Settlement::new(1, 100, 1, Arc::default());
    settlement.set_building_productivity_calculator(
        Some(calculator(0, 0, 3)),
        &test_map(),
        &mut test_finder(),
    );
    let mut building =
        Building::new(BuildingId(1), Position { x: 1, y: 0 }, definition(2, 2, 3)).unwrap();
    building.set_target_load(1.0).unwrap();
    settlement
        .add_building(building, &test_map(), &mut test_finder())
        .unwrap();
    settlement.deposit(ResourceType::Wood, 2);
    let report = settlement.process_turn(&test_map(), &mut test_finder());
    assert_eq!(settlement.inventory().amount(ResourceType::Wood), 1);
    assert_eq!(settlement.inventory().amount(ResourceType::Stone), 1);
    assert_eq!(report.inventory.production_consumed[&ResourceType::Wood], 1);
    assert!(report.inventory.is_balanced());
}

#[test]
fn two_settlements_keep_independent_productivity_configuration() {
    let mut first = settlement_at(Position { x: 20, y: 0 });
    let mut second = settlement_at(Position { x: 20, y: 0 });
    first.set_building_productivity_calculator(
        Some(calculator(0, 10, 30)),
        &test_map(),
        &mut test_finder(),
    );
    assert!(
        first
            .process_turn(&test_map(), &mut test_finder())
            .inventory
            .is_balanced()
    );
    assert!(
        second
            .process_turn(&test_map(), &mut test_finder())
            .inventory
            .is_balanced()
    );
    assert_eq!(first.inventory().amount(ResourceType::Stone), 10);
    assert_eq!(second.inventory().amount(ResourceType::Stone), 20);
}

#[test]
fn map_changes_update_output_using_the_current_astar_route_each_turn() {
    let mut map = core_map::Map::new(3, 3, vec![test_area::Area::new(0.5, 0.5); 9], vec![0; 9]);
    let mut finder = test_path::PathFinder::new(3, 3);
    let mut settlement = Settlement::new(1, 200, 10, Arc::default());
    let context = BuildingProductivityCalculator::new(
        Position { x: 0, y: 1 },
        TravelCostEfficiencyPolicy::new(20, 36).unwrap(),
    );
    settlement.set_building_productivity_calculator(Some(context), &map, &mut finder);
    let mut building = Building::new(
        BuildingId(7),
        Position { x: 2, y: 1 },
        definition(10, 10, 20),
    )
    .unwrap();
    building.set_target_load(1.0).unwrap();
    settlement
        .add_building(building, &map, &mut finder)
        .unwrap();
    settlement.deposit(ResourceType::Wood, 30);

    let first = settlement.process_turn(&map, &mut finder);
    assert_eq!(first.inventory.produced[&ResourceType::Stone], 20);
    assert!(first.inventory.is_balanced());
    map.terrain[4].elevation_val = 0.0;
    assert_eq!(
        context.travel_cost_to(Position { x: 2, y: 1 }, &map, &mut finder),
        Some(28)
    );
    let detour = settlement.process_turn(&map, &mut finder);
    assert_eq!(detour.inventory.produced[&ResourceType::Stone], 10);
    assert_eq!(
        detour.inventory.production_consumed[&ResourceType::Wood],
        10
    );
    assert!(detour.inventory.is_balanced());
    for y in 0..3 {
        map.terrain[y * 3 + 1].elevation_val = 0.0;
    }
    let blocked = settlement.process_turn(&map, &mut finder);
    assert_eq!(
        settlement
            .buildings()
            .next()
            .unwrap()
            .location_productivity(),
        0.0
    );
    assert_eq!(settlement.inventory().amount(ResourceType::Stone), 30);
    assert_eq!(
        blocked.inventory.production_consumed[&ResourceType::Wood],
        10
    );
    assert!(blocked.inventory.is_balanced());
}

#[test]
fn uphill_routes_reduce_productivity_at_the_same_geometric_distance() {
    let mut map = core_map::Map::new(2, 1, vec![test_area::Area::new(0.5, 0.5); 2], vec![0; 2]);
    let mut finder = test_path::PathFinder::new(2, 1);
    let context = BuildingProductivityCalculator::new(
        Position { x: 0, y: 0 },
        TravelCostEfficiencyPolicy::new(10, 100).unwrap(),
    );
    assert_eq!(
        context.productivity_at(Position { x: 1, y: 0 }, &map, &mut finder),
        1.0
    );
    map.terrain[1].elevation_val = 0.501;
    assert!(context.productivity_at(Position { x: 1, y: 0 }, &map, &mut finder) < 1.0);
    assert_eq!(
        context.productivity_at(Position { x: -1, y: 0 }, &map, &mut finder),
        0.0
    );
    assert_eq!(
        context.productivity_at(Position { x: 2, y: 0 }, &map, &mut finder),
        0.0
    );
    let invalid_capital =
        BuildingProductivityCalculator::new(Position { x: -1, y: 0 }, context.policy());
    assert_eq!(
        invalid_capital.productivity_at(Position { x: 1, y: 0 }, &map, &mut finder),
        0.0
    );
}

#[test]
fn changing_toml_thresholds_changes_production_for_the_same_route() {
    let map = test_map();
    let mut finder = test_finder();
    let mut settlement = settlement_at(Position { x: 2, y: 0 });
    for (zero_cost, output) in [(30, 10), (50, 15)] {
        let source = format!(
            "[economy.building_productivity]\nfull_productivity_cost = 10\nzero_productivity_cost = {zero_cost}"
        );
        let config = core::configuration::Configuration::from_toml(&source).unwrap();
        settlement.set_building_productivity_calculator(
            Some(BuildingProductivityCalculator::new(
                Position { x: 0, y: 0 },
                config.economy.building_productivity,
            )),
            &map,
            &mut finder,
        );
        let report = settlement.process_turn(&map, &mut finder);
        assert_eq!(report.inventory.produced[&ResourceType::Stone], output);
        assert_eq!(
            report.inventory.production_consumed[&ResourceType::Wood],
            10
        );
        assert!(report.inventory.is_balanced());
    }
}
