use simulation_core::ids::SettlementId;
use std::collections::HashMap;
use std::sync::Arc;

use simulation_core::buildings::{Building, definition::BuildingDefinition};
use simulation_core::economy::ResourceType::Wood;
use simulation_core::ids::{BuildingId, ProjectId};
use simulation_core::settlement::Settlement;
use simulation_core::settlement::projects::{ProjectCost, ProjectError};
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::area::Area;
use simulation_core::world::territory::{
    BuildingReference, ProjectReference, TerritoryError, TerritoryManager, TileOccupancy,
};
use simulation_core::world::{Map, Position, Province, World, WorldError};

const SITE: Position = Position { x: 0, y: 0 };

fn definition(tier: u8) -> Arc<BuildingDefinition> {
    Arc::new(BuildingDefinition {
        id: "house".into(),
        name: "House".into(),
        tier,
        max_workers: 0,
        behaviors: vec![],
        construction_cost: ProjectCost::default(),
    })
}

fn cost() -> ProjectCost {
    ProjectCost {
        needed_resources: HashMap::from([(Wood, 3)]),
        turn_cost: 2,
    }
}

fn world() -> World {
    let mut world = World::new(
        Map::new(2, 1, vec![Area::new(0.5, 0.5); 2], vec![10, 20]),
        vec![Province::new(10), Province::new(20)],
    );
    for (province, id) in [(10, 1), (20, 2)] {
        let mut settlement = Settlement::new(SettlementId(id), 100, 0, Arc::default());
        settlement.deposit(Wood, 10);
        world.add_settlement(province, settlement).unwrap();
    }
    world
}

#[test]
fn canceling_new_construction_releases_the_tile_and_allows_id_reuse() {
    let mut world = world();
    world
        .queue_construction(SettlementId(1), ProjectId(7), SITE, definition(1), cost())
        .unwrap();
    world.cancel_project(SettlementId(1), ProjectId(7)).unwrap();
    assert_eq!(
        world.tile_at(SITE).unwrap().occupancy(),
        TileOccupancy::Empty
    );
    assert!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .active_projects()
            .is_empty()
    );
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Wood),
        10
    );
    world
        .queue_construction(SettlementId(1), ProjectId(7), SITE, definition(1), cost())
        .unwrap();
    assert_eq!(
        world.tile_at(SITE).unwrap().occupancy(),
        TileOccupancy::Construction(ProjectReference {
            settlement_id: SettlementId(1),
            project_id: ProjectId(7)
        })
    );
}

#[test]
fn cancellation_after_partial_payment_preserves_spent_materials_and_stops_work() {
    let mut world = world();
    let mut finder = PathFinder::new(2, 1);
    world
        .queue_construction(SettlementId(1), ProjectId(7), SITE, definition(1), cost())
        .unwrap();
    world
        .process_settlement_turn(SettlementId(1), &mut finder)
        .unwrap();
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Wood),
        7
    );
    world.cancel_project(SettlementId(1), ProjectId(7)).unwrap();
    let report = world
        .process_settlement_turn(SettlementId(1), &mut finder)
        .unwrap();
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Wood),
        7
    );
    assert!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .buildings()
            .next()
            .is_none()
    );
    assert!(report.projects.is_empty());
    assert!(report.inventory.is_balanced());
    assert_eq!(
        world.tile_at(SITE).unwrap().occupancy(),
        TileOccupancy::Empty
    );
}

#[test]
fn canceling_an_upgrade_preserves_the_existing_building_and_occupancy() {
    let mut world = world();
    let mut finder = PathFinder::new(2, 1);
    let original = definition(1);
    world
        .add_building(
            SettlementId(1),
            Building::new(BuildingId(9), SITE, original.clone()).unwrap(),
            &mut finder,
        )
        .unwrap();
    world
        .queue_upgrade(
            SettlementId(1),
            ProjectId(7),
            BuildingId(9),
            definition(2),
            cost(),
        )
        .unwrap();
    world
        .process_settlement_turn(SettlementId(1), &mut finder)
        .unwrap();
    world.cancel_project(SettlementId(1), ProjectId(7)).unwrap();
    world
        .process_settlement_turn(SettlementId(1), &mut finder)
        .unwrap();
    let settlement = world.settlement(SettlementId(1)).unwrap();
    let building = settlement.buildings().next().unwrap();
    assert!(std::ptr::eq(building.definition(), original.as_ref()));
    assert_eq!(building.position(), SITE);
    assert_eq!(
        world.tile_at(SITE).unwrap().occupancy(),
        TileOccupancy::Building(BuildingReference {
            settlement_id: SettlementId(1),
            building_id: BuildingId(9)
        })
    );
    assert_eq!(settlement.inventory().amount(Wood), 7);
    assert!(settlement.active_projects().is_empty());
}

#[test]
fn missing_repeated_and_cross_settlement_cancellations_do_not_change_other_projects() {
    let mut world = world();
    world
        .queue_construction(SettlementId(1), ProjectId(7), SITE, definition(1), cost())
        .unwrap();
    let before = world.tile_at(SITE).unwrap();
    assert_eq!(
        world.cancel_project(SettlementId(99), ProjectId(7)),
        Err(WorldError::MissingSettlement(SettlementId(99)))
    );
    assert_eq!(
        world.cancel_project(SettlementId(2), ProjectId(7)),
        Err(WorldError::Project(ProjectError::MissingProject(
            ProjectId(7)
        )))
    );
    assert_eq!(
        world.cancel_project(SettlementId(1), ProjectId(8)),
        Err(WorldError::Project(ProjectError::MissingProject(
            ProjectId(8)
        )))
    );
    assert_eq!(world.tile_at(SITE).unwrap(), before);
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .active_projects()
            .len(),
        1
    );
    world.cancel_project(SettlementId(1), ProjectId(7)).unwrap();
    assert_eq!(
        world.cancel_project(SettlementId(1), ProjectId(7)),
        Err(WorldError::Project(ProjectError::MissingProject(
            ProjectId(7)
        )))
    );
    assert_eq!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .inventory()
            .amount(Wood),
        10
    );
}

#[test]
fn cancellation_does_not_require_the_original_map_membership() {
    let mut world = world();
    world
        .queue_construction(SettlementId(1), ProjectId(7), SITE, definition(1), cost())
        .unwrap();
    world.map.territory[0] = 20;
    world.cancel_project(SettlementId(1), ProjectId(7)).unwrap();
    assert_eq!(
        world.tile_at(SITE).unwrap().occupancy(),
        TileOccupancy::Empty
    );
    assert!(
        world
            .settlement(SettlementId(1))
            .unwrap()
            .active_projects()
            .is_empty()
    );
}

#[test]
fn mismatched_reservation_preserves_the_project_and_other_occupant() {
    let map = Map::new(1, 1, vec![Area::new(0.5, 0.5)], vec![10]);
    let mut territory = TerritoryManager::default();
    let mut settlement = Settlement::new(SettlementId(1), 100, 0, Arc::default());
    settlement
        .queue_construction(
            ProjectId(7),
            SITE,
            definition(1),
            cost(),
            &map,
            &mut territory,
        )
        .unwrap();
    territory
        .cancel_construction(
            SITE,
            ProjectReference {
                settlement_id: SettlementId(1),
                project_id: ProjectId(7),
            },
        )
        .unwrap();
    let other = ProjectReference {
        settlement_id: SettlementId(2),
        project_id: ProjectId(7),
    };
    territory.reserve_construction(SITE, other).unwrap();
    assert!(matches!(
        settlement.cancel_project(ProjectId(7), &mut territory),
        Err(ProjectError::Territory(
            TerritoryError::ReservationMismatch { .. }
        ))
    ));
    assert_eq!(settlement.active_projects().len(), 1);
    assert_eq!(
        settlement.active_projects()[0]
            .remaining_cost()
            .needed_resources[&Wood],
        3
    );
    assert_eq!(
        territory.tile_at(SITE).occupancy(),
        TileOccupancy::Construction(other)
    );
}
