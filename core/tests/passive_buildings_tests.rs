mod common;

use common::material;
use simulation_core::buildings::Building;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::buildings::passive::{
    BuildingPlacementLimit, BuildingRuleError, BuildingUpgradeRule, PassiveBuildingEffects,
    PassiveBuildingRules, TradeLimitBonuses,
};
use simulation_core::ids::{BuildingId, ProjectId, SettlementId};
use simulation_core::settlement::Settlement;
use simulation_core::settlement::projects::{ProjectCost, ProjectError};
use simulation_core::simulation::Simulation;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::area::Area;
use simulation_core::world::territory::TileOccupancy;
use simulation_core::world::{Map, Position, Province, World, WorldError};
use std::sync::Arc;

const OWNER: SettlementId = SettlementId(1);

fn position(x: i32) -> Position {
    Position { x, y: 0 }
}

fn world() -> World {
    let mut world = World::new(
        Map::new(8, 1, vec![Area::new(0.5, 0.5); 8], vec![1; 8]),
        vec![Province::new(1)],
    );
    world
        .add_settlement_at(
            1,
            position(0),
            Settlement::new(OWNER, 100, 1, Arc::default()),
        )
        .unwrap();
    world
}

fn blueprint(
    id: &str,
    effects: PassiveBuildingEffects,
    limit: Option<(&str, u32)>,
    upgrade: BuildingUpgradeRule,
) -> Arc<BuildingDefinition> {
    Arc::new(
        BuildingDefinition::new(id, id, 1, 0, vec![], ProjectCost::default())
            .unwrap()
            .with_passive_rules(PassiveBuildingRules {
                effects,
                limit: limit.map(|(group, maximum)| BuildingPlacementLimit {
                    group: group.to_owned(),
                    maximum,
                }),
                upgrade,
            }),
    )
}

fn advance(world: World) -> World {
    let mut simulation = Simulation::new(world).unwrap();
    simulation.step().unwrap();
    simulation.into_world()
}

fn build(
    world: &mut World,
    project_id: u32,
    x: i32,
    definition: &Arc<BuildingDefinition>,
) -> Result<(), WorldError> {
    world.queue_construction(
        OWNER,
        ProjectId(project_id),
        position(x),
        definition.clone(),
        ProjectCost::default(),
    )
}

#[test]
fn passive_warehouse_and_market_stack_and_demolition_preserves_stock() {
    let mut world = world();
    let warehouse = blueprint(
        "warehouse",
        PassiveBuildingEffects {
            warehouse_capacity: 2000,
            ..Default::default()
        },
        None,
        BuildingUpgradeRule::Unrestricted,
    );
    let market = blueprint(
        "market",
        PassiveBuildingEffects {
            trade: TradeLimitBonuses {
                caravan_slots: 2,
                offer_slots: 4,
                caravan_cargo: 400,
                open_offer_units: 800,
            },
            ..Default::default()
        },
        None,
        BuildingUpgradeRule::Unrestricted,
    );
    let mut finder = PathFinder::new(8, 1);
    for (id, definition) in [(0, &warehouse), (1, &warehouse), (2, &market), (3, &market)] {
        world
            .add_building(
                OWNER,
                Building::new(BuildingId(id), position(id as i32), definition.clone()),
                &mut finder,
            )
            .unwrap();
    }
    let producer = Arc::new(
        BuildingDefinition::new(
            "producer",
            "Producer",
            1,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(material("wood"), 300)],
            }],
            ProjectCost::default(),
        )
        .unwrap(),
    );
    let mut building = Building::new(BuildingId(4), position(4), producer);
    building.set_target_load(1.0).unwrap();
    world.add_building(OWNER, building, &mut finder).unwrap();

    let mut world = advance(world);
    let settlement = world.settlement(OWNER).unwrap();
    assert_eq!(settlement.inventory().capacity(), 4100);
    assert_eq!(settlement.pending_inventory().capacity(), 4100);
    assert_eq!(
        settlement.trade_limit_bonuses(),
        TradeLimitBonuses {
            caravan_slots: 4,
            offer_slots: 8,
            caravan_cargo: 800,
            open_offer_units: 1600,
        }
    );
    let stored = settlement.inventory().amount(material("wood"));
    assert!(stored > 100);

    for id in [0, 1, 2] {
        world.demolish_building(OWNER, BuildingId(id)).unwrap();
        assert_eq!(
            world.tile_at(position(id as i32)).unwrap().occupancy(),
            TileOccupancy::Empty
        );
    }

    let settlement = world.settlement(OWNER).unwrap();
    assert_eq!(settlement.inventory().capacity(), 100);
    assert_eq!(settlement.pending_inventory().capacity(), 100);
    assert_eq!(settlement.inventory().amount(material("wood")), stored);
    assert_eq!(
        settlement.trade_limit_bonuses(),
        market.passive_rules().effects.trade
    );
}

#[test]
fn monument_limit_reserves_active_projects_and_demolition_frees_a_slot() {
    let mut world = world();
    let monument = blueprint(
        "monument",
        PassiveBuildingEffects {
            happiness: 5,
            ..Default::default()
        },
        Some(("monuments", 4)),
        BuildingUpgradeRule::Unrestricted,
    );
    for index in 0..4 {
        build(&mut world, index as u32, index, &monument).unwrap();
    }
    assert_eq!(
        world.settlement(OWNER).unwrap().monument_happiness_bonus(),
        0
    );
    assert_eq!(
        build(&mut world, 4, 4, &monument),
        Err(WorldError::Project(ProjectError::BuildingRule(
            BuildingRuleError::LimitReached {
                group: "monuments".to_owned(),
                maximum: 4,
            }
        )))
    );
    let mut world = advance(world);
    assert_eq!(
        world.settlement(OWNER).unwrap().monument_happiness_bonus(),
        20
    );

    world.demolish_building(OWNER, BuildingId(0)).unwrap();
    assert_eq!(
        world.settlement(OWNER).unwrap().monument_happiness_bonus(),
        15
    );
    build(&mut world, 4, 4, &monument).unwrap();
    let world = advance(world);
    assert_eq!(
        world.settlement(OWNER).unwrap().monument_happiness_bonus(),
        20
    );
}

#[test]
fn walls_upgrade_one_level_at_a_time_and_replace_the_defense_bonus() {
    let mut world = world();
    let wall = |id, bonus, rule| {
        blueprint(
            id,
            PassiveBuildingEffects {
                defense_basis_points: bonus,
                ..Default::default()
            },
            Some(("walls", 1)),
            rule,
        )
    };
    let level_one = wall("wall_1", 2500, BuildingUpgradeRule::ConstructionOnly);
    let level_two = wall(
        "wall_2",
        5000,
        BuildingUpgradeRule::From("wall_1".to_owned()),
    );
    let level_three = wall(
        "wall_3",
        10000,
        BuildingUpgradeRule::From("wall_2".to_owned()),
    );
    let upgrade = |world: &mut World, project, definition: &Arc<BuildingDefinition>| {
        world.queue_upgrade(
            OWNER,
            ProjectId(project),
            BuildingId(0),
            definition.clone(),
            ProjectCost::default(),
        )
    };
    let requires = |id: &str| {
        Err(WorldError::Project(ProjectError::BuildingRule(
            BuildingRuleError::UpgradeRequires {
                previous_type: id.to_owned(),
            },
        )))
    };

    assert_eq!(build(&mut world, 0, 0, &level_two), requires("wall_1"));
    build(&mut world, 0, 0, &level_one).unwrap();
    let mut world = advance(world);
    assert_eq!(
        world
            .province_fortification_defense_basis_points(1)
            .unwrap(),
        2500
    );
    assert_eq!(upgrade(&mut world, 1, &level_three), requires("wall_2"));
    upgrade(&mut world, 1, &level_two).unwrap();
    assert_eq!(
        upgrade(&mut world, 2, &level_two),
        Err(WorldError::Project(ProjectError::BuildingAlreadyUpgrading(
            BuildingId(0)
        )))
    );
    let mut world = advance(world);
    assert_eq!(
        world
            .province_fortification_defense_basis_points(1)
            .unwrap(),
        5000
    );
    upgrade(&mut world, 2, &level_three).unwrap();
    let mut world = advance(world);
    assert_eq!(
        world
            .province_fortification_defense_basis_points(1)
            .unwrap(),
        10000
    );

    world.demolish_building(OWNER, BuildingId(0)).unwrap();
    assert_eq!(
        world
            .province_fortification_defense_basis_points(1)
            .unwrap(),
        0
    );
}
