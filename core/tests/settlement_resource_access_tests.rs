use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::economy::ResourceType::{Labour, Stone, Wood};
use simulation_core::ids::{BuildingTypeId, SettlementId};
use simulation_core::settlement::{Settlement, projects::ProjectCost};
use simulation_core::simulation::{
    ActionError, ActionOutcome, SettlementAction, SettlementDecision, Simulation,
};
use simulation_core::world::territory::TileOccupancy;
use simulation_core::world::{Map, Position, Province, World, WorldError, terrain::area::Area};
use std::collections::HashMap;
use std::sync::Arc;

const OWNER: SettlementId = SettlementId(1);
const NEIGHBOUR: SettlementId = SettlementId(2);

fn at(x: i32) -> Position {
    Position { x, y: 0 }
}

fn scenario() -> World {
    let land = Area::new(0.5, 0.5);
    let ocean = Area::new(0.1, 0.5);
    let mut world = World::new(
        Map::new(
            5,
            1,
            vec![land, land, land, land, ocean],
            vec![10, 10, 10, 20, 20],
        ),
        vec![Province::new(10), Province::new(20)],
    );
    let quarry = BuildingDefinition::new(
        "quarry",
        "Quarry",
        1,
        1,
        vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(Stone, 2)],
        }],
        ProjectCost {
            needed_resources: HashMap::from([(Wood, 2)]),
            turn_cost: 1,
        },
    )
    .unwrap()
    .requiring_deposit(Stone)
    .unwrap();
    let mut catalog = BuildingCatalog::default();
    catalog.register(quarry);
    world.set_building_catalog(catalog);
    let mut owner = Settlement::new(OWNER, 100, 1, Arc::default());
    owner.deposit(Wood, 6);
    world.add_settlement(10, owner).unwrap();
    world
        .add_settlement(20, Settlement::new(NEIGHBOUR, 100, 1, Arc::default()))
        .unwrap();
    world.place_deposit(at(2), Stone).unwrap();
    world.place_deposit(at(3), Stone).unwrap();
    world
}

fn construct(x: i32) -> SettlementAction {
    SettlementAction::Construct {
        building_type: BuildingTypeId::new("quarry"),
        position: at(x),
    }
}

#[test]
fn deposits_follow_province_membership_and_reject_invalid_placement() {
    let mut world = scenario();
    assert_eq!(
        world.controlled_positions(OWNER).unwrap(),
        vec![at(0), at(1), at(2)]
    );
    assert_eq!(
        world.controlled_positions(NEIGHBOUR).unwrap(),
        vec![at(3), at(4)]
    );
    assert_eq!(
        world.controlled_positions(SettlementId(99)),
        Err(WorldError::MissingSettlement(SettlementId(99)))
    );
    assert_eq!(
        world.accessible_deposits(OWNER).unwrap(),
        vec![(at(2), Stone)]
    );
    assert_eq!(
        world.accessible_deposits(NEIGHBOUR).unwrap(),
        vec![(at(3), Stone)]
    );
    assert_eq!(
        world.place_deposit(at(1), Labour),
        Err(WorldError::InvalidDepositResource(Labour))
    );
    assert_eq!(
        world.place_deposit(at(4), Stone),
        Err(WorldError::DepositOnOcean(at(4)))
    );
    assert_eq!(
        world.place_deposit(at(2), Wood),
        Err(WorldError::DepositAlreadyExists(at(2)))
    );
    assert_eq!(world.inspect_tile(at(2)).unwrap().deposit, Some(Stone));
    world.set_tile_province(at(2), 20).unwrap();
    assert!(world.accessible_deposits(OWNER).unwrap().is_empty());
    assert_eq!(
        world.accessible_deposits(NEIGHBOUR).unwrap(),
        vec![(at(2), Stone), (at(3), Stone)]
    );
}

#[test]
fn only_an_owned_deposit_supports_quarry_construction_and_production() {
    let mut simulation = Simulation::new(scenario()).unwrap();
    let observation = simulation.observe(OWNER).unwrap();
    assert_eq!(observation.territory[2].deposit, Some(Stone));
    assert_eq!(observation.building_types[0].required_deposit, Some(Stone));
    assert!(
        observation
            .territory
            .iter()
            .all(|tile| tile.position != at(3))
    );

    simulation
        .queue_decision(SettlementDecision::new(
            OWNER,
            vec![construct(1), construct(3)],
        ))
        .unwrap();
    let turn = simulation.step().unwrap();
    let results = &turn.decisions[0].results;
    assert_eq!(
        results[0].outcome,
        ActionOutcome::Rejected {
            error: ActionError::RequiredDepositMissing {
                position: at(1),
                resource: Stone
            }
        }
    );
    assert_eq!(
        results[1].outcome,
        ActionOutcome::Rejected {
            error: ActionError::OutsideOwnProvince { position: at(3) }
        }
    );
    assert!(
        simulation
            .settlement(OWNER)
            .unwrap()
            .active_projects()
            .is_empty()
    );
    assert_eq!(
        simulation
            .settlement(OWNER)
            .unwrap()
            .inventory()
            .amount(Wood),
        6
    );
    for position in [at(1), at(3)] {
        assert_eq!(
            simulation.world().tile_at(position).unwrap().occupancy(),
            TileOccupancy::Empty
        );
    }

    simulation
        .queue_decision(SettlementDecision::new(OWNER, vec![construct(2)]))
        .unwrap();
    let turn = simulation.step().unwrap();
    assert!(matches!(
        turn.decisions[0].results[0].outcome,
        ActionOutcome::Applied { .. }
    ));
    let site = simulation.world().inspect_tile(at(2)).unwrap();
    assert_eq!(site.deposit, Some(Stone));
    let quarry_id = site.building.unwrap().instance_id();
    simulation
        .queue_decision(SettlementDecision::new(
            OWNER,
            vec![SettlementAction::SetBuildingWorkers {
                building_id: quarry_id,
                workers: 1,
            }],
        ))
        .unwrap();
    let turn = simulation.step().unwrap();
    assert_eq!(turn.settlements[&OWNER].inventory.produced[&Stone], 2);

    let mut world = simulation.into_world();
    world.set_tile_province(at(2), 20).unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    let turn = simulation.step().unwrap();
    assert!(
        !turn.settlements[&OWNER]
            .inventory
            .produced
            .contains_key(&Stone)
    );
    assert_eq!(
        simulation
            .settlement(OWNER)
            .unwrap()
            .inventory()
            .amount(Stone),
        2
    );
    assert_eq!(
        simulation
            .settlement(OWNER)
            .unwrap()
            .buildings()
            .next()
            .unwrap()
            .current_workers(),
        0
    );
}
