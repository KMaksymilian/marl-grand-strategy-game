use std::collections::HashMap;
use std::sync::Arc;

use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::BuildingDefinition;
use simulation_core::buildings::{
    Building,
    productivity::{BuildingProductivityCalculator, TravelCostEfficiencyPolicy},
};
use simulation_core::economy::ResourceType::Wood;
use simulation_core::ids::BuildingTypeId;
use simulation_core::ids::{BuildingId, ProjectId, SettlementId};
use simulation_core::settlement::{NeedRegistry, NeedTier, Settlement, projects::ProjectCost};
use simulation_core::simulation::{SettlementAction, SettlementDecision, Simulation, TurnPhase};
use simulation_core::world::territory::{ProjectReference, TileOccupancy};
use simulation_core::world::{Map, Position, Province, World, WorldError, terrain::area::Area};
use simulation_core::world::{
    pathfinding::path_finder::PathFinder, settlement_environment::AStarTravelCosts,
};

fn world() -> World {
    World::new(
        Map::new(
            5,
            1,
            vec![Area::new(0.5, 0.5); 5],
            vec![100, 101, 102, 103, 104],
        ),
        (100..105).rev().map(Province::new).collect(),
    )
}

#[test]
fn capitals_are_validated_before_registration_and_stay_in_their_provinces() {
    let land = Area::new(0.5, 0.5);
    let ocean = Area::new(0.1, 0.5);
    let mut world = World::new(
        Map::new(
            4,
            1,
            vec![land, land, ocean, ocean],
            vec![100, 101, 100, 102],
        ),
        (100..=102).map(Province::new).collect(),
    );
    let capital = Position { x: 0, y: 0 };
    let foreign = Position { x: 1, y: 0 };
    let sea = Position { x: 2, y: 0 };
    let outside = Position { x: 4, y: 0 };
    for (position, error) in [
        (outside, WorldError::InvalidMapPosition(outside)),
        (sea, WorldError::CapitalOnOcean(sea)),
        (
            foreign,
            WorldError::CapitalOutsideProvince {
                position: foreign,
                expected_province: 100,
                actual_province: 101,
            },
        ),
    ] {
        assert_eq!(
            world.initialize_settlement_at(100, position, 10, 0, Arc::default()),
            Err(error)
        );
    }
    assert_eq!(
        world.initialize_settlement(102, 10, 0, Arc::default()),
        Err(WorldError::NoCapitalSite(102))
    );
    assert!(world.settlement_registry().is_empty());
    let first = world
        .initialize_settlement_at(100, capital, 10, 0, Arc::default())
        .unwrap();
    assert_eq!(first, SettlementId(0));
    assert_eq!(
        world.initialize_settlement_at(100, capital, 10, 0, Arc::default()),
        Err(WorldError::ProvinceOccupied {
            province_id: 100,
            settlement_id: first
        })
    );
    assert_eq!(
        world.set_tile_terrain(capital, ocean),
        Err(WorldError::CapitalSiteProtected(capital))
    );
    assert_eq!(
        world.set_tile_province(capital, 101),
        Err(WorldError::CapitalSiteProtected(capital))
    );

    let mut simulation = Simulation::new(world).unwrap();
    let second = simulation
        .initialize_settlement_at(101, foreign, 10, 0, Arc::default())
        .unwrap();
    assert_eq!(second, SettlementId(1));
    simulation.step().unwrap();
    assert_eq!(
        simulation.settlement(first).unwrap().capital(),
        Some(capital)
    );
    assert_eq!(
        simulation.settlement(second).unwrap().capital(),
        Some(foreign)
    );
    assert_eq!(simulation.world().province_at(capital), Ok(100));
    assert_eq!(
        simulation.world().map().terrain[0].elevation_val,
        land.elevation_val
    );
}

#[test]
fn registered_capital_is_the_origin_of_building_productivity() {
    let mut world = World::new(
        Map::new(3, 1, vec![Area::new(0.5, 0.5); 3], vec![100; 3]),
        vec![Province::new(100)],
    );
    let first_land = Position { x: 0, y: 0 };
    let capital = Position { x: 2, y: 0 };
    let id = SettlementId(1);
    let mut settlement = Settlement::new(id, 100, 1, Arc::default());
    let mut finder = PathFinder::new(3, 1);
    let policy = TravelCostEfficiencyPolicy::new(0, 10).unwrap();
    settlement.set_building_productivity_calculator(
        Some(BuildingProductivityCalculator::new(first_land, policy)),
        &mut AStarTravelCosts::new(world.map(), &mut finder),
    );
    world.add_settlement_at(100, capital, settlement).unwrap();
    let definition = Arc::new(
        BuildingDefinition::new("farm", "Farm", 1, 1, vec![], ProjectCost::default()).unwrap(),
    );
    for (building_id, position) in [(BuildingId(1), capital), (BuildingId(2), first_land)] {
        world
            .add_building(
                id,
                Building::new(building_id, position, definition.clone()),
                &mut finder,
            )
            .unwrap();
    }
    let mut simulation = Simulation::new(world).unwrap();
    simulation.step().unwrap();
    let multipliers: Vec<_> = simulation
        .settlement(id)
        .unwrap()
        .buildings()
        .map(Building::location_productivity)
        .collect();
    assert_eq!(multipliers, [1.0, 0.0]);
}

#[test]
fn all_public_registry_views_and_processing_have_the_same_stable_order() {
    let mut simulation = Simulation::new(world()).unwrap();
    for (province, id) in [(102, 42), (100, 7), (101, 99)] {
        simulation
            .add_settlement(
                province,
                Settlement::new(SettlementId(id), 100, 0, Arc::default()),
            )
            .unwrap();
    }
    let first = simulation
        .initialize_settlement(103, 100, 0, Arc::default())
        .unwrap();
    assert_eq!(first, SettlementId(0));
    let expected = vec![
        SettlementId(0),
        SettlementId(7),
        SettlementId(42),
        SettlementId(99),
    ];
    assert_eq!(
        simulation
            .settlement_registry()
            .iter()
            .map(Settlement::id)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        simulation
            .settlements()
            .map(Settlement::id)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        simulation
            .world()
            .settlements()
            .map(Settlement::id)
            .collect::<Vec<_>>(),
        expected
    );
    let report = simulation.step().unwrap();
    for phase in [
        TurnPhase::Decisions,
        TurnPhase::Preparation,
        TurnPhase::Allocation,
        TurnPhase::Consumption,
        TurnPhase::Construction,
        TurnPhase::Production,
        TurnPhase::EconomicUpdates,
    ] {
        assert_eq!(
            report
                .execution
                .iter()
                .filter(|event| event.phase == phase)
                .map(|event| event.settlement_id)
                .collect::<Vec<_>>(),
            expected
        );
    }
    // Registration between turns joins the same registry and its sorted traversal.
    assert_eq!(
        simulation.initialize_settlement(104, 100, 0, Arc::default()),
        Ok(SettlementId(1))
    );
    assert_eq!(
        simulation
            .step()
            .unwrap()
            .settlements
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![
            SettlementId(0),
            SettlementId(1),
            SettlementId(7),
            SettlementId(42),
            SettlementId(99)
        ]
    );
}

#[test]
fn failed_initialization_does_not_consume_an_id_or_register_an_orphan() {
    let mut simulation = Simulation::new(world()).unwrap();
    assert_eq!(
        simulation.initialize_settlement(999, 100, 0, Arc::default()),
        Err(WorldError::MissingProvince(999))
    );
    assert!(simulation.settlement_registry().is_empty());
    let first = simulation
        .initialize_settlement(100, 100, 3, Arc::default())
        .unwrap();
    assert_eq!(first, SettlementId(0));
    assert_eq!(
        simulation.initialize_settlement(100, 100, 99, Arc::default()),
        Err(WorldError::ProvinceOccupied {
            province_id: 100,
            settlement_id: first
        })
    );
    assert_eq!(simulation.settlement_registry().len(), 1);
    assert_eq!(
        simulation.initialize_settlement(101, 100, 0, Arc::default()),
        Ok(SettlementId(1))
    );
    assert_eq!(simulation.settlement(first).unwrap().population(), 3);
}

#[test]
fn duplicate_registration_and_unknown_references_do_not_change_existing_state() {
    let mut simulation = Simulation::new(world()).unwrap();
    let id = simulation
        .initialize_settlement(100, 100, 3, Arc::default())
        .unwrap();
    assert_eq!(
        simulation.add_settlement(101, Settlement::new(id, 1, 99, Arc::default())),
        Err(WorldError::DuplicateSettlement(id))
    );
    assert_eq!(simulation.world().province_settlement(101), Ok(None));
    assert_eq!(simulation.settlement(id).unwrap().population(), 3);
    assert_eq!(simulation.settlement_registry().len(), 1);
    let missing = SettlementId(999);
    assert!(
        matches!(simulation.settlement(missing), Err(WorldError::MissingSettlement(found)) if found == missing)
    );
    assert!(simulation.settlement_registry().get(missing).is_none());
    assert_eq!(
        simulation.queue_decision(SettlementDecision::new(
            missing,
            vec![SettlementAction::CancelProject {
                project_id: ProjectId(1)
            }]
        )),
        Err(WorldError::MissingSettlement(missing))
    );
    let report = simulation.step().unwrap();
    assert_eq!(report.settlements.len(), 1);
    assert!(report.decisions.is_empty());
}

#[test]
fn initialized_population_and_needs_are_used_by_the_economy() {
    let mut simulation = Simulation::new(world()).unwrap();
    let needs = Arc::new(NeedRegistry::new(vec![NeedTier {
        min_population: 1,
        needs_per_capita: HashMap::from([(Wood, 1)]),
    }]));
    let id = simulation
        .initialize_settlement(100, 100, 4, needs)
        .unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(simulation.settlement(id).unwrap().population(), 4);
    assert_eq!(
        report.settlements[&id].consumption.missing_resources,
        vec![Wood]
    );
    assert_eq!(
        report.settlements[&id].consumption.satisfaction_percent,
        0.0
    );
}

#[test]
fn registry_and_id_allocation_survive_recovering_the_world() {
    let mut simulation = Simulation::new(world()).unwrap();
    let first = simulation
        .initialize_settlement(100, 100, 3, Arc::default())
        .unwrap();
    simulation.step().unwrap();
    let world = simulation.into_world();
    let mut simulation = Simulation::new(world).unwrap();
    let second = simulation
        .initialize_settlement(101, 100, 4, Arc::default())
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(simulation.settlement_registry().len(), 2);
    assert_eq!(simulation.settlement(first).unwrap().population(), 3);
    assert_eq!(simulation.settlement(second).unwrap().population(), 4);
}

#[test]
fn registry_owns_the_same_persistent_economic_and_spatial_state_across_turns() {
    let mut world = world();
    let id = SettlementId(42);
    let mut settlement = Settlement::new(id, 100, 0, Arc::default());
    settlement.deposit(Wood, 5);
    world.add_settlement(100, settlement).unwrap();
    let mut catalog = BuildingCatalog::default();
    catalog.register(
        BuildingDefinition::new(
            "house",
            "House",
            1,
            0,
            vec![],
            ProjectCost {
                needed_resources: HashMap::from([(Wood, 2)]),
                turn_cost: 2,
            },
        )
        .unwrap(),
    );
    world.set_building_catalog(catalog);
    let mut simulation = Simulation::new(world).unwrap();
    let site = Position { x: 0, y: 0 };
    simulation
        .queue_decision(SettlementDecision::new(
            id,
            vec![SettlementAction::Construct {
                building_type: BuildingTypeId::new("house"),
                position: site,
            }],
        ))
        .unwrap();
    simulation.step().unwrap();
    let settlement = simulation.settlement_registry().get(id).unwrap();
    assert!(std::ptr::eq(
        settlement,
        simulation.world().settlement(id).unwrap()
    ));
    assert!(std::ptr::eq(settlement, simulation.settlement(id).unwrap()));
    assert_eq!(settlement.inventory().amount(Wood), 3);
    assert_eq!(
        settlement.active_projects()[0].remaining_cost().turn_cost,
        1
    );
    assert_eq!(
        simulation.world().tile_at(site).unwrap().occupancy(),
        TileOccupancy::Construction(ProjectReference {
            settlement_id: id,
            project_id: ProjectId(1)
        })
    );
    simulation.step().unwrap();
    let settlement = simulation.settlement(id).unwrap();
    assert_eq!(settlement.inventory().amount(Wood), 3);
    assert!(settlement.active_projects().is_empty());
    let building = settlement.buildings().next().unwrap();
    assert_eq!(building.position(), site);
    assert!(std::ptr::eq(
        simulation
            .world()
            .inspect_tile(site)
            .unwrap()
            .building
            .unwrap(),
        building
    ));
    assert_eq!(simulation.world().province_settlement(100), Ok(Some(id)));
}
