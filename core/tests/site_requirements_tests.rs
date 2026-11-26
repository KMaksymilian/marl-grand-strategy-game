mod common;

use common::material;
use simulation_core::buildings::Building;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use simulation_core::settlement::{
    Settlement,
    projects::{ProjectCost, ProjectError},
};
use simulation_core::simulation::{
    ActionError, ActionOutcome, SettlementAction, SettlementDecision, Simulation,
};
use simulation_core::spatial::site::SiteRequirements;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::territory::TerritoryError;
use simulation_core::world::{Map, Position, Province, World, WorldError, terrain::area::Area};
use std::sync::Arc;

const OWNER: SettlementId = SettlementId(1);
const WIDTH: usize = 5;
const HEIGHT: usize = 3;
const COASTAL: &str = "site_requirements = [{ fact = 'tile.coastal', op = 'eq', value = true }]";

fn at(x: i32, y: i32) -> Position {
    Position { x, y }
}

fn world() -> World {
    let mut terrain = vec![Area::new(0.5, 0.45); WIDTH * HEIGHT];
    terrain[2] = Area::new(0.5, 0.55);
    terrain[WIDTH + 2] = Area::new(0.1, 0.45);
    let provinces = [1, 1, 1, 1, 2].repeat(HEIGHT);
    let mut world = World::new(
        Map::new(WIDTH, HEIGHT, terrain, provinces),
        vec![Province::new(1), Province::new(2)],
    );
    world
        .add_settlement_at(
            1,
            at(0, 0),
            Settlement::new(OWNER, 1000, 10, Arc::default()),
        )
        .unwrap();
    world
}

fn blueprint(world: &World, id: &str, rules: &str, output: &str) -> BuildingDefinition {
    let requirements =
        SiteRequirements::from_toml(rules, world.resource_catalog(), world.site_fact_types())
            .unwrap();
    BuildingDefinition::new(
        id,
        id,
        1,
        1,
        vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(material(output), 100)],
        }],
        ProjectCost {
            turn_cost: 1,
            ..ProjectCost::default()
        },
    )
    .unwrap()
    .with_site_requirements(requirements)
}

fn construct(id: &str, position: Position) -> SettlementAction {
    SettlementAction::Construct {
        building_type: BuildingTypeId::new(id),
        position,
    }
}

fn queue(simulation: &mut Simulation, actions: Vec<SettlementAction>) {
    simulation
        .queue_decision(SettlementDecision::new(OWNER, actions))
        .unwrap();
}

fn add_staffed(
    world: &mut World,
    id: u32,
    position: Position,
    definition: Arc<BuildingDefinition>,
) {
    let mut building = Building::new(BuildingId(id), position, definition);
    building.set_target_load(1.0).unwrap();
    world
        .add_building(OWNER, building, &mut PathFinder::new(WIDTH, HEIGHT))
        .unwrap();
}

#[test]
fn the_same_coastal_specification_validates_construction_and_upgrades() {
    let mut world = world();
    let hut = Arc::new(blueprint(&world, "hut", "", "wood"));
    add_staffed(&mut world, 1, at(0, 1), hut.clone());
    add_staffed(&mut world, 2, at(2, 2), hut);
    let mut catalog = BuildingCatalog::default();
    catalog.register(blueprint(&world, "wharf", COASTAL, "fish"));
    world.set_building_catalog(catalog);
    let mut simulation = Simulation::new(world).unwrap();
    let upgrade = |id| SettlementAction::Upgrade {
        building_id: BuildingId(id),
        building_type: BuildingTypeId::new("wharf"),
    };
    queue(
        &mut simulation,
        vec![
            construct("wharf", at(0, 2)), // Inland
            construct("wharf", at(1, 0)), // Diagonal to ocean
            construct("wharf", at(2, 1)), // Ocean
            upgrade(1),
            upgrade(2),
            construct("wharf", at(2, 0)),
        ],
    );

    let report = simulation.step().unwrap();
    let results = &report.decisions[0].results;
    for (index, position) in [(0, at(0, 2)), (1, at(1, 0)), (3, at(0, 1))] {
        assert_eq!(
            results[index].outcome,
            ActionOutcome::Rejected {
                error: ActionError::CoastRequired { position },
            }
        );
    }
    assert_eq!(
        results[2].outcome,
        ActionOutcome::Rejected {
            error: ActionError::OceanTile { position: at(2, 1) },
        },
    );
    assert!(results[4].is_applied());
    assert!(results[5].is_applied());

    for position in [at(2, 0), at(2, 2)] {
        let tile = simulation.world().inspect_tile(position).unwrap();
        assert_eq!(tile.building.unwrap().definition().id(), "wharf");
    }
}

#[test]
fn a_wharf_stops_requesting_workers_when_its_ocean_neighbor_disappears() {
    let mut world = world();
    let wharf = Arc::new(blueprint(&world, "wharf", COASTAL, "fish"));
    add_staffed(&mut world, 1, at(2, 0), wharf);
    let mut simulation = Simulation::new(world).unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.settlements[&OWNER].inventory.produced[&material("fish")],
        100
    );

    let mut world = simulation.into_world();
    world
        .set_tile_terrain(at(2, 1), Area::new(0.5, 0.45))
        .unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    let report = simulation.step().unwrap();
    assert!(
        !report.settlements[&OWNER]
            .inventory
            .produced
            .contains_key(&material("fish"))
    );
    assert_eq!(
        simulation
            .settlement(OWNER)
            .unwrap()
            .buildings()
            .next()
            .unwrap()
            .current_workers(),
        0,
    );
}

#[test]
fn a_province_deposit_is_distinct_from_a_tile_deposit_and_requires_the_right_ore() {
    let mut world = world();
    world.place_deposit(at(4, 0), material("iron_ore")).unwrap();
    world
        .place_deposit(at(3, 0), material("copper_ore"))
        .unwrap();
    let mut catalog = BuildingCatalog::default();
    catalog.register(blueprint(
        &world,
        "mine",
        "site_requirements = [{ fact = 'province.deposits', op = 'contains', value = 'iron_ore' }]",
        "iron_ore",
    ));
    catalog.register(blueprint(
        &world,
        "quarry",
        "site_requirements = [{ fact = 'tile.deposits', op = 'contains', value = 'iron_ore' }]",
        "iron_ore",
    ));
    world.set_building_catalog(catalog);
    let mut simulation = Simulation::new(world).unwrap();
    queue(&mut simulation, vec![construct("mine", at(1, 0))]);
    let report = simulation.step().unwrap();
    assert_eq!(
        report.decisions[0].results[0].outcome,
        ActionOutcome::Rejected {
            error: ActionError::ProvinceDepositRequired {
                position: at(1, 0),
                resource: material("iron_ore"),
            },
        },
    );

    let mut world = simulation.into_world();
    world.place_deposit(at(3, 2), material("iron_ore")).unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    queue(
        &mut simulation,
        vec![
            construct("quarry", at(1, 0)),
            construct("mine", at(1, 0)),
            construct("quarry", at(3, 2)),
        ],
    );
    let report = simulation.step().unwrap();
    let results = &report.decisions[0].results;
    assert_eq!(
        results[0].outcome,
        ActionOutcome::Rejected {
            error: ActionError::RequiredDepositMissing {
                position: at(1, 0),
                resource: material("iron_ore"),
            },
        },
    );
    assert!(results[1].is_applied());
    assert!(results[2].is_applied());
}

#[test]
fn the_biome_specification_controls_yield_and_rejects_work_after_terrain_changes() {
    let mut world = world();
    let farm = Arc::new(blueprint(
        &world,
        "plantation",
        "biome_efficiency = { grassland = 100, forest = 75 }",
        "grain",
    ));
    let desert_site = at(0, 2);
    world
        .set_tile_terrain(desert_site, Area::new(0.5, 0.35))
        .unwrap();
    assert_eq!(
        world.queue_construction(
            OWNER,
            ProjectId(1),
            desert_site,
            farm.clone(),
            farm.construction_cost().clone(),
        ),
        Err(WorldError::Project(ProjectError::Territory(
            TerritoryError::ForbiddenBiome(desert_site)
        ))),
    );
    add_staffed(&mut world, 1, at(1, 0), farm.clone());
    add_staffed(&mut world, 2, at(2, 0), farm);
    let mut simulation = Simulation::new(world).unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.settlements[&OWNER].inventory.produced[&material("grain")],
        175
    );

    let mut world = simulation.into_world();
    world
        .set_tile_terrain(at(1, 0), Area::new(0.5, 0.35))
        .unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    let report = simulation.step().unwrap();
    assert_eq!(
        report.settlements[&OWNER].inventory.produced[&material("grain")],
        75
    );
    let blocked = simulation
        .settlement(OWNER)
        .unwrap()
        .buildings()
        .find(|building| building.instance_id() == BuildingId(1))
        .unwrap();
    assert_eq!(blocked.current_workers(), 0);
}

#[test]
fn a_configured_fact_combines_with_a_derived_fact_without_a_new_evaluator() {
    let mut world = world();
    world
        .load_site_facts_toml(
            r#"
[fact_types]
"tile.soil_fertility" = "integer"

[[tiles]]
x = 2
y = 0
[tiles.facts]
"tile.soil_fertility" = 60

[[tiles]]
x = 2
y = 2
[tiles.facts]
"tile.soil_fertility" = 59
"#,
        )
        .unwrap();
    assert!(
        world
            .load_site_facts_toml(
                r#"
[fact_types]
"tile.soil_fertility" = "integer"

[[tiles]]
x = 2
y = 0
[tiles.facts]
"tile.soil_fertility" = 0

[[tiles]]
x = 99
y = 0
[tiles.facts]
"tile.soil_fertility" = 100
"#,
            )
            .is_err()
    );
    let parse = |rules| {
        SiteRequirements::from_toml(rules, world.resource_catalog(), world.site_fact_types())
    };
    assert!(
        parse("site_requirements = [{ fact = 'tile.unknown', op = 'eq', value = true }]").is_err()
    );
    assert!(
        parse("site_requirements = [{ fact = 'tile.coastal', op = 'gte', value = 1 }]").is_err()
    );
    assert!(
        parse("site_requirements = [{ fact = 'tile.soil_fertility', op = 'eq', value = '60' }]")
            .is_err()
    );
    let mut catalog = BuildingCatalog::default();
    catalog.register(blueprint(
        &world,
        "coastal_farm",
        "site_requirements = [{ fact = 'tile.coastal', op = 'eq', value = true }, { fact = 'tile.soil_fertility', op = 'gte', value = 60 }]",
        "grain",
    ));
    world.set_building_catalog(catalog);
    let mut simulation = Simulation::new(world).unwrap();
    queue(
        &mut simulation,
        vec![
            construct("coastal_farm", at(2, 2)),
            construct("coastal_farm", at(1, 1)),
            construct("coastal_farm", at(2, 0)),
        ],
    );
    let report = simulation.step().unwrap();
    let results = &report.decisions[0].results;
    for (result, position) in results.iter().zip([at(2, 2), at(1, 1)]) {
        assert_eq!(
            result.outcome,
            ActionOutcome::Rejected {
                error: ActionError::SiteRequirementNotMet {
                    position,
                    fact: "tile.soil_fertility".to_owned(),
                },
            },
        );
    }
    assert!(results[2].is_applied());
}
