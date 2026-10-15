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
use core::domain::buildings::factory::{
    BuildingBehavior, BuildingDefinition, BuildingFactory, DefinitionError,
};
use core::domain::economy::resource::ResourceType::{self, Grain, Stone, Wood};
use core::domain::economy::storage_order::StorageOrderError;
use core::domain::ids::{BuildingId, ProjectId};
use core::domain::settlement::core::{BuildingCommandError, Settlement};
use core::domain::settlement::projects::{ProjectCost, ProjectError};
use std::sync::Arc;

fn definition(
    inputs: Vec<(ResourceType, u32)>,
    outputs: Vec<(ResourceType, u32)>,
) -> BuildingDefinition {
    BuildingDefinition {
        id: "mill".into(),
        name: "Mill".into(),
        tier: 1,
        max_workers: 1,
        behaviors: vec![BuildingBehavior::Production { inputs, outputs }],
        construction_cost: ProjectCost::default(),
    }
}

#[test]
fn rejects_totals_across_duplicate_entries_and_recipes() {
    let mut blueprint = definition(vec![(Wood, u32::MAX)], vec![]);
    blueprint.behaviors.push(BuildingBehavior::Production {
        inputs: vec![(Wood, 1)],
        outputs: vec![],
    });
    assert_eq!(
        blueprint.validate(),
        Err(DefinitionError::InputOverflow(Wood))
    );
    assert_eq!(
        definition(vec![], vec![(Stone, u32::MAX), (Stone, 1)]).validate(),
        Err(DefinitionError::OutputOverflow(Stone))
    );
    assert!(
        definition(vec![(Wood, u32::MAX)], vec![(Stone, u32::MAX)])
            .validate()
            .is_ok()
    );
}

#[test]
fn invalid_registration_and_upgrade_preserve_existing_definition() {
    let mut factory = BuildingFactory::default();
    factory
        .register(definition(vec![], vec![(Grain, 1)]))
        .unwrap();
    assert_eq!(
        factory.register(definition(vec![], vec![(Grain, u32::MAX), (Grain, 1)])),
        Err(DefinitionError::OutputOverflow(Grain))
    );
    let blueprint = factory.get_blueprint("mill").unwrap();
    let mut building =
        Building::new(BuildingId(1), Position { x: 0, y: 0 }, blueprint.clone()).unwrap();
    assert_eq!(
        building.upgrade(Arc::new(definition(
            vec![(Wood, u32::MAX), (Wood, 1)],
            vec![]
        ))),
        Err(DefinitionError::InputOverflow(Wood))
    );
    assert!(std::ptr::eq(building.definition(), blueprint.as_ref()));
}

#[test]
fn invalid_efficiencies_do_not_change_existing_target() {
    let mut settlement = Settlement::new(1, 10, 1, Arc::default());
    let mut building = Building::new(
        BuildingId(1),
        Position { x: 0, y: 0 },
        Arc::new(definition(vec![], vec![(Grain, 1)])),
    )
    .unwrap();
    building.set_target_load(0.5).unwrap();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
        assert!(building.set_target_load(value).is_err());
        assert_eq!(building.target_load(), 0.5);
    }
    settlement
        .add_building(building, &test_map(), &mut test_finder())
        .unwrap();
    assert_eq!(
        settlement.set_building_load(BuildingId(1), f32::NAN),
        Err(BuildingCommandError::InvalidLoad)
    );
    assert_eq!(
        settlement.set_building_load(BuildingId(9), 1.0),
        Err(BuildingCommandError::MissingBuilding(BuildingId(9)))
    );
    assert_eq!(settlement.buildings().next().unwrap().target_load(), 0.5);
}

#[test]
fn invalid_project_is_rejected_before_queueing_or_spending() {
    let mut settlement = Settlement::new(1, 10, 1, Arc::default());
    settlement.deposit(Wood, 10);
    let invalid = Arc::new(definition(vec![], vec![(Grain, u32::MAX), (Grain, 1)]));
    assert_eq!(
        settlement.queue_construction(
            ProjectId(1),
            Position { x: 0, y: 0 },
            invalid,
            ProjectCost::default()
        ),
        Err(ProjectError::InvalidDefinition(
            DefinitionError::OutputOverflow(Grain)
        ))
    );
    assert!(settlement.active_projects().is_empty());
    assert_eq!(settlement.inventory().amount(Wood), 10);
    assert!(
        settlement
            .process_turn(&test_map(), &mut test_finder())
            .inventory
            .is_balanced()
    );
}

#[test]
fn variable_length_storage_order_is_validated_atomically() {
    let mut settlement = Settlement::new(1, 10, 1, Arc::default());
    settlement
        .set_storage_order(vec![Grain, Wood, Stone])
        .unwrap();
    assert_eq!(
        settlement.set_storage_order(vec![Wood, Stone]),
        Err(StorageOrderError::MissingMaterial(Grain))
    );
    assert_eq!(
        settlement.set_storage_order(Vec::<ResourceType>::new()),
        Err(StorageOrderError::MissingMaterial(Wood))
    );
    assert_eq!(settlement.storage_order(), &[Grain, Wood, Stone]);
    assert_eq!(
        settlement.storage_order().len(),
        ResourceType::MATERIALS.len()
    );
}

#[test]
fn rejects_invalid_staffing_data_and_direct_construction() {
    let mut blueprint = definition(vec![], vec![]);
    blueprint.behaviors = vec![BuildingBehavior::StatModifier {
        stat: core::domain::settlement::stat::StatType::HousingCapacity,
        base_amount: u32::MAX,
        per_worker_amount: 1,
    }];
    assert_eq!(blueprint.validate(), Err(DefinitionError::StatOverflow));
    assert!(Building::new(BuildingId(1), Position { x: 0, y: 0 }, Arc::new(blueprint)).is_err());
    assert_eq!(
        definition(vec![(ResourceType::Labour, 1)], vec![]).validate(),
        Err(DefinitionError::LabourInRecipe)
    );
}
