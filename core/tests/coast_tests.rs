mod common;

use common::material;
use simulation_core::buildings::Building;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::BuildingDefinition;
use simulation_core::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use simulation_core::settlement::projects::{ProjectCost, ProjectError};
use simulation_core::settlement::{BuildingCommandError, Settlement};
use simulation_core::simulation::{
    ActionError, ActionOutcome, SettlementAction, SettlementDecision, Simulation,
};
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::{
    area::Area, biome::Biome, elevation::Elevation, moisture::Moisture,
};
use simulation_core::world::territory::{TerritoryError, TileOccupancy};
use simulation_core::world::{Map, Position, Province, World, WorldError};
use std::collections::HashMap;
use std::sync::Arc;

const OWNER: SettlementId = SettlementId(1);

fn at(x: i32, y: i32) -> Position {
    Position { x, y }
}

fn land() -> Area {
    Area::new(0.5, 0.5)
}

fn ocean() -> Area {
    Area::new(0.1, 0.5)
}

fn definition(id: &str, coastal: bool) -> Arc<BuildingDefinition> {
    let definition = BuildingDefinition::new(
        id,
        id,
        1,
        0,
        vec![],
        ProjectCost {
            needed_resources: HashMap::from([(material("wood"), 2)]),
            turn_cost: 1,
        },
    )
    .unwrap();
    Arc::new(if coastal {
        definition.requiring_coast()
    } else {
        definition
    })
}

fn scenario() -> World {
    let mut world = World::new(
        Map::new(
            5,
            2,
            vec![
                land(),
                land(),
                land(),
                land(),
                land(),
                land(),
                land(),
                ocean(),
                land(),
                ocean(),
            ],
            vec![10, 10, 10, 20, 20, 10, 10, 10, 20, 20],
        ),
        vec![Province::new(10), Province::new(20)],
    );
    let mut settlement = Settlement::new(OWNER, 20, 0, Arc::default());
    settlement.deposit(material("wood"), 8);
    world.add_settlement_at(10, at(0, 0), settlement).unwrap();
    let mut catalog = BuildingCatalog::default();
    catalog.register(Arc::try_unwrap(definition("wharf", true)).unwrap());
    world.set_building_catalog(catalog);
    world
        .add_building(
            OWNER,
            Building::new(BuildingId(1), at(0, 0), definition("hut", false)),
            &mut PathFinder::new(5, 2),
        )
        .unwrap();
    world
}

#[test]
fn coast_requires_land_and_a_cardinal_water_neighbor() {
    for neighbor in [at(1, 0), at(0, 1), at(2, 1), at(1, 2)] {
        let mut terrain = vec![land(); 9];
        terrain[neighbor.y as usize * 3 + neighbor.x as usize] = ocean();
        let map = Map::new(3, 3, terrain, vec![10; 9]);
        assert!(map.is_coastal(at(1, 1)), "{neighbor:?}");
        assert!(!map.is_coastal(neighbor));
    }
    let mut terrain = vec![land(); 9];
    terrain[0] = ocean();
    let map = Map::new(3, 3, terrain, vec![10; 9]);
    assert!(!map.is_coastal(at(1, 1)));
    assert!(!map.is_coastal(at(-1, 0)));
    assert!(!map.is_coastal(at(3, 1)));
    assert!(!Map::new(1, 1, vec![land()], vec![10]).is_coastal(at(0, 0)));
}

#[test]
fn all_land_biome_mappings_and_ocean_classification_are_preserved() {
    use Biome::*;
    let expected = [
        [Desert, Grassland, Forest, Rainforest],
        [Desert, Grassland, Forest, Forest],
        [Hills, Hills, Taiga, Taiga],
        [Tundra, Tundra, Snow, Snow],
    ];
    for (elevation, row) in [
        Elevation::Lowland,
        Elevation::Plains,
        Elevation::Highland,
        Elevation::Mountain,
    ]
    .into_iter()
    .zip(expected)
    {
        for (moisture, expected) in [
            Moisture::Dry,
            Moisture::Normal,
            Moisture::Humid,
            Moisture::Wet,
        ]
        .into_iter()
        .zip(row)
        {
            assert_eq!(Biome::determine(&elevation, &moisture), expected);
            assert_eq!(Biome::determine(&Elevation::Ocean, &moisture), Ocean);
        }
    }
}

#[test]
fn direct_building_and_project_commands_enforce_coast_without_spending() {
    let mut world = scenario();
    let coastal = definition("wharf", true);
    let position = at(1, 0);
    let cost = coastal.construction_cost().clone();
    assert!(coastal.requires_coast());
    assert_eq!(
        world.queue_construction(OWNER, ProjectId(1), position, coastal.clone(), cost.clone()),
        Err(WorldError::Project(ProjectError::Territory(
            TerritoryError::CoastRequired(position)
        ))),
    );
    assert_eq!(
        world.add_building(
            OWNER,
            Building::new(BuildingId(2), position, coastal.clone()),
            &mut PathFinder::new(5, 2),
        ),
        Err(WorldError::Building(BuildingCommandError::Territory(
            TerritoryError::CoastRequired(position)
        ))),
    );
    assert_eq!(
        world.queue_upgrade(
            OWNER,
            ProjectId(1),
            BuildingId(1),
            coastal.clone(),
            cost.clone()
        ),
        Err(WorldError::Project(ProjectError::Territory(
            TerritoryError::CoastRequired(at(0, 0))
        ))),
    );
    assert_eq!(
        world
            .settlement(OWNER)
            .unwrap()
            .inventory()
            .amount(material("wood")),
        8
    );
    assert!(
        world
            .settlement(OWNER)
            .unwrap()
            .active_projects()
            .is_empty()
    );
    assert_eq!(
        world.tile_at(position).unwrap().occupancy(),
        TileOccupancy::Empty
    );

    world
        .add_building(
            OWNER,
            Building::new(BuildingId(2), at(2, 0), definition("shore_hut", false)),
            &mut PathFinder::new(5, 2),
        )
        .unwrap();
    world
        .queue_upgrade(OWNER, ProjectId(1), BuildingId(2), coastal, cost)
        .unwrap();
    assert_eq!(world.settlement(OWNER).unwrap().active_projects().len(), 1);
}

#[test]
fn agent_commands_report_coast_and_ownership_and_accept_an_owned_shore() {
    let mut simulation = Simulation::new(scenario()).unwrap();
    let observation = simulation.observe(OWNER).unwrap();
    assert!(
        observation
            .building_types
            .iter()
            .find(|building| building.building_type == BuildingTypeId::new("wharf"))
            .unwrap()
            .requires_coast
    );
    for (position, expected) in [(at(1, 0), false), (at(2, 0), true)] {
        assert_eq!(
            observation
                .territory
                .iter()
                .find(|tile| tile.position == position)
                .unwrap()
                .coastal,
            expected,
        );
    }
    let construct = |position| SettlementAction::Construct {
        building_type: BuildingTypeId::new("wharf"),
        position,
    };
    simulation
        .queue_decision(SettlementDecision::new(
            OWNER,
            vec![
                construct(at(1, 0)),
                construct(at(3, 0)),
                SettlementAction::Upgrade {
                    building_id: BuildingId(1),
                    building_type: BuildingTypeId::new("wharf"),
                },
                construct(at(2, 0)),
            ],
        ))
        .unwrap();
    let report = simulation.step().unwrap();
    let results = &report.decisions[0].results;
    for (index, error) in [
        (0, ActionError::CoastRequired { position: at(1, 0) }),
        (1, ActionError::OutsideOwnProvince { position: at(3, 0) }),
        (2, ActionError::CoastRequired { position: at(0, 0) }),
    ] {
        assert_eq!(results[index].outcome, ActionOutcome::Rejected { error });
    }
    assert!(results[3].is_applied());
    assert_eq!(
        report.settlements[&OWNER].inventory.project_consumed[&material("wood")],
        2
    );
    assert_eq!(
        simulation
            .world()
            .settlement(OWNER)
            .unwrap()
            .inventory()
            .amount(material("wood")),
        6
    );
}
