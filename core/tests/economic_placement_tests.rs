mod common;

use common::material;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::economy::ResourceCatalog;
use simulation_core::ids::{BuildingTypeId, ProjectId, SettlementId};
use simulation_core::settlement::{Settlement, projects::ProjectError};
use simulation_core::simulation::{
    ActionError, ActionOutcome, SettlementAction, SettlementDecision, Simulation,
};
use simulation_core::spatial::{Position, TerritoryError, TileOccupancy};
use simulation_core::world::{Map, Province, World, terrain::area::Area};
use std::sync::Arc;

const OWNER: SettlementId = SettlementId(1);
const CAPITAL: Position = at(0, 0);
const OWN_ORE: Position = at(1, 0);
const MINE_SITE: Position = at(2, 0);
const FOREIGN_ORE: Position = at(4, 0);
const DESERT: Position = at(0, 1);
const INLAND: Position = at(1, 1);
const COAST: Position = at(2, 1);
const SEA: Position = at(3, 1);

const fn at(x: i32, y: i32) -> Position {
    Position { x, y }
}

fn ocean() -> Area {
    Area::new(0.1, 0.45)
}

fn scenario() -> World {
    let mut terrain = vec![Area::new(0.5, 0.45); 10];
    terrain[5] = Area::new(0.5, 0.35);
    terrain[8] = ocean();
    let mut world = World::new(
        Map::new(5, 2, terrain, vec![10, 10, 10, 10, 20, 10, 10, 10, 10, 20]),
        vec![Province::new(10), Province::new(20)],
    );
    let buildings = BuildingCatalog::from_toml(
        include_str!("../../buildings.toml"),
        ResourceCatalog::standard(),
    )
    .unwrap();
    world.set_building_catalog(buildings);
    let mut settlement = Settlement::new(OWNER, 5_000, 25, Arc::default());
    for resource in ["wood", "stone", "clay", "limestone"] {
        settlement.deposit(material(resource), 1_000);
    }
    world.add_settlement_at(10, CAPITAL, settlement).unwrap();
    world
        .place_deposit(FOREIGN_ORE, material("iron_ore"))
        .unwrap();
    world
        .place_deposit(MINE_SITE, material("copper_ore"))
        .unwrap();
    world
}

fn construct(id: &str, position: Position) -> SettlementAction {
    SettlementAction::Construct {
        building_type: BuildingTypeId::new(id),
        position,
    }
}

#[test]
fn waterfront_requires_an_owned_land_coast_and_plantation_rejects_desert() {
    let mut simulation = Simulation::new(scenario()).unwrap();
    simulation
        .queue_decision(SettlementDecision::new(
            OWNER,
            vec![
                construct("waterfront", INLAND),
                construct("waterfront", SEA),
                construct("plantation", DESERT),
                construct("waterfront", COAST),
            ],
        ))
        .unwrap();

    let turn = simulation.step().unwrap();
    let results = &turn.decisions[0].results;
    for (index, error) in [
        (0, ActionError::CoastRequired { position: INLAND }),
        (1, ActionError::OceanTile { position: SEA }),
        (2, ActionError::ForbiddenBiome { position: DESERT }),
    ] {
        assert_eq!(results[index].outcome, ActionOutcome::Rejected { error });
    }
    assert!(results[3].is_applied());
    assert!(matches!(
        simulation.world().tile_at(COAST).unwrap().occupancy(),
        TileOccupancy::Construction(_)
    ));
}

#[test]
fn mine_uses_the_selected_ore_anywhere_on_land_in_its_own_province() {
    let mut simulation = Simulation::new(scenario()).unwrap();
    let mine = || SettlementDecision::new(OWNER, vec![construct("mine_iron_ore", MINE_SITE)]);
    simulation.queue_decision(mine()).unwrap();
    let turn = simulation.step().unwrap();
    assert_eq!(
        turn.decisions[0].results[0].outcome,
        ActionOutcome::Rejected {
            error: ActionError::RequiredDepositMissing {
                position: MINE_SITE,
                resource: material("iron_ore"),
            },
        }
    );

    let mut world = simulation.into_world();
    world.place_deposit(OWN_ORE, material("iron_ore")).unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    simulation.queue_decision(mine()).unwrap();
    assert!(simulation.step().unwrap().decisions[0].results[0].is_applied());
    simulation.step().unwrap();
    simulation.step().unwrap();
    let building_id = simulation
        .world()
        .inspect_tile(MINE_SITE)
        .unwrap()
        .building
        .unwrap()
        .instance_id();
    simulation
        .queue_decision(SettlementDecision::new(
            OWNER,
            vec![SettlementAction::SetBuildingWorkers {
                building_id,
                workers: 25,
            }],
        ))
        .unwrap();
    let turn = simulation.step().unwrap();
    assert_eq!(
        turn.settlements[&OWNER].inventory.produced[&material("iron_ore")],
        100
    );

    let mut world = simulation.into_world();
    world.set_tile_terrain(OWN_ORE, ocean()).unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    let turn = simulation.step().unwrap();
    assert!(
        !turn.settlements[&OWNER]
            .inventory
            .produced
            .contains_key(&material("iron_ore"))
    );
}

#[test]
fn queued_mine_losing_its_province_deposit_is_rejected_before_spending() {
    let mut world = scenario();
    world.place_deposit(OWN_ORE, material("iron_ore")).unwrap();
    let mine = world
        .building_catalog()
        .get_blueprint("mine_iron_ore")
        .unwrap();
    world
        .queue_construction(
            OWNER,
            ProjectId(9),
            MINE_SITE,
            mine.clone(),
            mine.construction_cost().clone(),
        )
        .unwrap();
    world.set_tile_province(OWN_ORE, 20).unwrap();

    let mut simulation = Simulation::new(world).unwrap();
    let turn = simulation.step().unwrap();
    let settlement = &turn.settlements[&OWNER];
    assert_eq!(
        settlement.projects[0].result,
        Err(ProjectError::Territory(
            TerritoryError::ProvinceDepositRequired(MINE_SITE)
        ))
    );
    assert!(settlement.inventory.project_consumed.is_empty());
    assert_eq!(
        simulation.world().tile_at(MINE_SITE).unwrap().occupancy(),
        TileOccupancy::Empty
    );
}
