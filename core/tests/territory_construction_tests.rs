use std::collections::HashMap;
use std::sync::Arc;

use simulation_core::buildings::{Building, definition::BuildingDefinition};
use simulation_core::economy::ResourceType;
use simulation_core::ids::{BuildingId, ProjectId};
use simulation_core::settlement::projects::{ProjectCompletion, ProjectCost, ProjectError};
use simulation_core::settlement::{BuildingCommandError, Settlement};
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::area::Area;
use simulation_core::world::territory::{
    BuildingReference, ProjectReference, TerritoryError, TileOccupancy,
};
use simulation_core::world::{Map, Position, Province, World, WorldError};

const SITE: Position = Position { x: 0, y: 0 };
const NEXT_SITE: Position = Position { x: 1, y: 0 };

fn definition() -> Arc<BuildingDefinition> {
    Arc::new(BuildingDefinition {
        id: "house".into(),
        name: "House".into(),
        tier: 1,
        max_workers: 0,
        behaviors: vec![],
        construction_cost: ProjectCost::default(),
    })
}

fn world() -> World {
    let mut world = World::new(
        Map::new(
            4,
            2,
            vec![Area::new(0.5, 0.5); 8],
            vec![10, 10, 20, 30, 10, 10, 20, 30],
        ),
        vec![Province::new(10), Province::new(20), Province::new(30)],
    );
    for (province, id) in [(10, 1), (20, 2)] {
        let mut settlement = Settlement::new(id, 100, 0, Arc::default());
        settlement.deposit(ResourceType::Wood, 5);
        world.add_settlement(province, settlement).unwrap();
    }
    world
}

fn project(settlement_id: u32, project_id: u32) -> TileOccupancy {
    TileOccupancy::Construction(ProjectReference {
        settlement_id,
        project_id: ProjectId(project_id),
    })
}

fn building(settlement_id: u32, building_id: u32) -> TileOccupancy {
    TileOccupancy::Building(BuildingReference {
        settlement_id,
        building_id: BuildingId(building_id),
    })
}

#[test]
fn construction_reserves_a_tile_until_materials_and_time_are_paid() {
    let mut world = world();
    let mut finder = PathFinder::new(4, 2);
    world
        .queue_construction(
            1,
            ProjectId(7),
            SITE,
            definition(),
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 2)]),
                turn_cost: 2,
            },
        )
        .unwrap();
    assert_eq!(world.tile_at(SITE).unwrap().occupancy(), project(1, 7));
    assert_eq!(
        world
            .settlement(1)
            .unwrap()
            .inventory()
            .amount(ResourceType::Wood),
        5
    );

    let first = world.process_settlement_turn(1, &mut finder).unwrap();
    assert!(first.projects.is_empty());
    assert!(first.inventory.is_balanced());
    assert_eq!(
        world
            .settlement(1)
            .unwrap()
            .inventory()
            .amount(ResourceType::Wood),
        3
    );
    assert_eq!(world.tile_at(SITE).unwrap().occupancy(), project(1, 7));

    let second = world.process_settlement_turn(1, &mut finder).unwrap();
    assert_eq!(
        second.projects[0].result,
        Ok(ProjectCompletion::BuildingConstructed(BuildingId(0)))
    );
    assert_eq!(world.tile_at(SITE).unwrap().occupancy(), building(1, 0));
    let settlement = world.settlement(1).unwrap();
    assert!(settlement.active_projects().is_empty());
    assert_eq!(settlement.buildings().next().unwrap().position(), SITE);
    assert!(second.inventory.is_balanced());
}

#[test]
fn invalid_sites_do_not_create_projects_or_spend_materials() {
    let mut world = world();
    world.map.terrain[0] = Area::new(0.1, 0.5);
    assert_eq!(
        world.queue_construction(1, ProjectId(1), SITE, definition(), ProjectCost::default()),
        Err(WorldError::Project(ProjectError::Territory(
            TerritoryError::OceanTile(SITE)
        )))
    );
    for position in [Position { x: -1, y: 0 }, Position { x: 4, y: 0 }] {
        assert_eq!(
            world.queue_construction(
                1,
                ProjectId(1),
                position,
                definition(),
                ProjectCost::default()
            ),
            Err(WorldError::InvalidMapPosition(position))
        );
    }
    for (x, province_id, registered_settlement) in [(2, 20, Some(2)), (3, 30, None)] {
        assert_eq!(
            world.queue_construction(
                1,
                ProjectId(1),
                Position { x, y: 0 },
                definition(),
                ProjectCost::default()
            ),
            Err(WorldError::SettlementOutsideProvince {
                province_id,
                settlement_id: 1,
                registered_settlement
            })
        );
    }
    assert_eq!(
        world.queue_construction(
            99,
            ProjectId(1),
            NEXT_SITE,
            definition(),
            ProjectCost::default()
        ),
        Err(WorldError::MissingSettlement(99))
    );
    let settlement = world.settlement(1).unwrap();
    assert!(settlement.active_projects().is_empty());
    assert_eq!(settlement.inventory().amount(ResourceType::Wood), 5);
    assert_eq!(
        world.tile_at(SITE).unwrap().occupancy(),
        TileOccupancy::Empty
    );
}

#[test]
fn duplicate_project_ids_and_occupied_tiles_do_not_leave_phantom_reservations() {
    let mut world = world();
    world
        .queue_construction(1, ProjectId(7), SITE, definition(), ProjectCost::default())
        .unwrap();
    assert_eq!(
        world.queue_construction(
            1,
            ProjectId(7),
            NEXT_SITE,
            definition(),
            ProjectCost::default()
        ),
        Err(WorldError::Project(ProjectError::DuplicateProject(
            ProjectId(7)
        )))
    );
    assert_eq!(
        world.tile_at(NEXT_SITE).unwrap().occupancy(),
        TileOccupancy::Empty
    );
    assert_eq!(
        world.queue_construction(1, ProjectId(8), SITE, definition(), ProjectCost::default()),
        Err(WorldError::Project(ProjectError::Territory(
            TerritoryError::Occupied {
                position: SITE,
                occupancy: project(1, 7)
            }
        )))
    );
    assert_eq!(world.settlement(1).unwrap().active_projects().len(), 1);

    world
        .process_settlement_turn(1, &mut PathFinder::new(4, 2))
        .unwrap();
    assert_eq!(
        world.queue_construction(1, ProjectId(8), SITE, definition(), ProjectCost::default()),
        Err(WorldError::Project(ProjectError::Territory(
            TerritoryError::Occupied {
                position: SITE,
                occupancy: building(1, 0)
            }
        )))
    );
    assert!(world.settlement(1).unwrap().active_projects().is_empty());
}

#[test]
fn direct_building_registration_cannot_bypass_spatial_checks() {
    let mut world = world();
    let mut finder = PathFinder::new(4, 2);
    world
        .queue_construction(1, ProjectId(7), SITE, definition(), ProjectCost::default())
        .unwrap();
    let create = |id, position| Building::new(BuildingId(id), position, definition()).unwrap();
    assert_eq!(
        world.add_building(1, create(3, SITE), &mut finder),
        Err(WorldError::Building(BuildingCommandError::Territory(
            TerritoryError::Occupied {
                position: SITE,
                occupancy: project(1, 7)
            }
        )))
    );
    assert!(world.settlement(1).unwrap().buildings().next().is_none());
    world
        .add_building(1, create(3, NEXT_SITE), &mut finder)
        .unwrap();
    assert_eq!(
        world.tile_at(NEXT_SITE).unwrap().occupancy(),
        building(1, 3)
    );
    assert_eq!(
        world.add_building(1, create(4, NEXT_SITE), &mut finder),
        Err(WorldError::Building(BuildingCommandError::Territory(
            TerritoryError::Occupied {
                position: NEXT_SITE,
                occupancy: building(1, 3),
            }
        )))
    );
    let spare = Position { x: 0, y: 1 };
    assert_eq!(
        world.add_building(1, create(3, spare), &mut finder),
        Err(WorldError::Building(
            BuildingCommandError::DuplicateBuilding(BuildingId(3))
        ))
    );
    assert_eq!(
        world.tile_at(spare).unwrap().occupancy(),
        TileOccupancy::Empty
    );
    assert!(matches!(
        world.add_building(1, create(4, Position { x: 2, y: 0 }), &mut finder),
        Err(WorldError::SettlementOutsideProvince { .. })
    ));
    assert!(matches!(
        world.add_building(1, create(4, Position { x: -1, y: 0 }), &mut finder),
        Err(WorldError::InvalidMapPosition(_))
    ));
    world.map.terrain[4] = Area::new(0.1, 0.5);
    assert_eq!(
        world.add_building(1, create(4, spare), &mut finder),
        Err(WorldError::Building(BuildingCommandError::Territory(
            TerritoryError::OceanTile(spare)
        )))
    );
    assert_eq!(world.settlement(1).unwrap().buildings().count(), 1);
}

#[test]
fn upgrades_keep_the_building_on_its_tile_throughout_the_project() {
    let mut world = world();
    let mut finder = PathFinder::new(4, 2);
    world
        .add_building(
            1,
            Building::new(BuildingId(9), SITE, definition()).unwrap(),
            &mut finder,
        )
        .unwrap();
    let mut upgraded = Arc::try_unwrap(definition()).unwrap();
    upgraded.tier = 2;
    world
        .queue_upgrade(
            1,
            ProjectId(7),
            BuildingId(9),
            Arc::new(upgraded),
            ProjectCost {
                turn_cost: 2,
                ..ProjectCost::default()
            },
        )
        .unwrap();
    for _ in 0..2 {
        assert_eq!(world.tile_at(SITE).unwrap().occupancy(), building(1, 9));
        world.process_settlement_turn(1, &mut finder).unwrap();
    }
    assert_eq!(world.tile_at(SITE).unwrap().occupancy(), building(1, 9));
    assert_eq!(
        world
            .settlement(1)
            .unwrap()
            .buildings()
            .next()
            .unwrap()
            .definition()
            .tier,
        2
    );
}

#[test]
fn different_settlements_can_use_the_same_local_project_and_building_ids() {
    let mut world = world();
    let second_site = Position { x: 2, y: 0 };
    let mut finder = PathFinder::new(4, 2);
    for (id, position) in [(1, SITE), (2, second_site)] {
        world
            .queue_construction(
                id,
                ProjectId(7),
                position,
                definition(),
                ProjectCost::default(),
            )
            .unwrap();
        assert_eq!(world.tile_at(position).unwrap().occupancy(), project(id, 7));
        world.process_settlement_turn(id, &mut finder).unwrap();
        assert_eq!(
            world.tile_at(position).unwrap().occupancy(),
            building(id, 0)
        );
    }
    assert_eq!(world.tile_at(SITE).unwrap().occupancy(), building(1, 0));
}

#[test]
fn flooded_construction_is_rejected_before_spending_and_releases_its_reservation() {
    let mut world = world();
    world
        .queue_construction(
            1,
            ProjectId(7),
            SITE,
            definition(),
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 2)]),
                turn_cost: 1,
            },
        )
        .unwrap();
    world.map.terrain[0] = Area::new(0.1, 0.5);
    let report = world
        .process_settlement_turn(1, &mut PathFinder::new(4, 2))
        .unwrap();
    assert_eq!(
        report.projects[0].result,
        Err(ProjectError::Territory(TerritoryError::OceanTile(SITE)))
    );
    assert_eq!(
        world
            .settlement(1)
            .unwrap()
            .inventory()
            .amount(ResourceType::Wood),
        5
    );
    assert!(world.settlement(1).unwrap().active_projects().is_empty());
    assert_eq!(
        world.tile_at(SITE).unwrap().occupancy(),
        TileOccupancy::Empty
    );
    assert!(report.inventory.is_balanced());
}

#[test]
fn changed_province_membership_stops_the_turn_before_spending() {
    let mut world = world();
    world
        .queue_construction(
            1,
            ProjectId(7),
            SITE,
            definition(),
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 2)]),
                turn_cost: 1,
            },
        )
        .unwrap();
    world.map.territory[0] = 20;
    assert!(matches!(
        world.process_settlement_turn(1, &mut PathFinder::new(4, 2)),
        Err(WorldError::SettlementOutsideProvince { .. })
    ));
    assert_eq!(
        world
            .settlement(1)
            .unwrap()
            .inventory()
            .amount(ResourceType::Wood),
        5
    );
    assert_eq!(world.tile_at(SITE).unwrap().occupancy(), project(1, 7));
    assert_eq!(world.settlement(1).unwrap().active_projects().len(), 1);
}

#[test]
fn invalid_project_cannot_release_another_settlements_reservation() {
    use simulation_core::world::territory::TerritoryManager;

    let map = Map::new(1, 1, vec![Area::new(0.5, 0.5)], vec![10]);
    let mut territory = TerritoryManager::default();
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    settlement.deposit(ResourceType::Wood, 5);
    settlement
        .queue_construction(
            ProjectId(7),
            SITE,
            definition(),
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 2)]),
                turn_cost: 1,
            },
            &map,
            &mut territory,
        )
        .unwrap();
    territory
        .cancel_construction(
            SITE,
            ProjectReference {
                settlement_id: 1,
                project_id: ProjectId(7),
            },
        )
        .unwrap();
    let other = ProjectReference {
        settlement_id: 2,
        project_id: ProjectId(7),
    };
    territory.reserve_construction(SITE, other).unwrap();

    let report = settlement.process_turn(&map, &mut PathFinder::new(1, 1), &mut territory);
    assert!(matches!(
        report.projects[0].result,
        Err(ProjectError::Territory(
            TerritoryError::ReservationMismatch { .. }
        ))
    ));
    assert!(settlement.active_projects().is_empty());
    assert_eq!(settlement.inventory().amount(ResourceType::Wood), 5);
    assert_eq!(
        territory.tile_at(SITE).occupancy(),
        TileOccupancy::Construction(other)
    );
}
