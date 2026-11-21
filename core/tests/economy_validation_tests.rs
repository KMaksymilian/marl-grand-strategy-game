mod common;
use common::material;
use simulation_core::ids::SettlementId;
use simulation_core::world::map as core_map;
use simulation_core::world::pathfinding::path_finder as test_path;
use simulation_core::world::settlement_environment::MapSettlementEnvironment;
use simulation_core::world::terrain::area as test_area;
use simulation_core::world::territory::TerritoryManager;

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
use simulation_core::buildings::Building;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::{
    BuildingBehavior, BuildingDefinition, DefinitionError,
};
use simulation_core::economy::storage_order::StorageOrderError;
use simulation_core::economy::{ResourceCatalog, ResourceType};
use simulation_core::ids::BuildingId;
use simulation_core::settlement::projects::ProjectCost;
use simulation_core::settlement::{BuildingCommandError, Settlement};
use simulation_core::world::coordinates::Position;
use std::sync::Arc;

fn definition(
    inputs: Vec<(ResourceType, u32)>,
    outputs: Vec<(ResourceType, u32)>,
) -> BuildingDefinition {
    BuildingDefinition::new(
        "mill",
        "Mill",
        1,
        1,
        vec![BuildingBehavior::Production { inputs, outputs }],
        ProjectCost::default(),
    )
    .unwrap()
}

fn try_definition(behaviors: Vec<BuildingBehavior>) -> Result<BuildingDefinition, DefinitionError> {
    BuildingDefinition::new("mill", "Mill", 1, 1, behaviors, ProjectCost::default())
}

#[test]
fn rejects_totals_across_duplicate_entries_and_recipes_at_creation() {
    assert_eq!(
        try_definition(vec![
            BuildingBehavior::Production {
                inputs: vec![(material("wood"), u32::MAX)],
                outputs: vec![]
            },
            BuildingBehavior::Production {
                inputs: vec![(material("wood"), 1)],
                outputs: vec![]
            },
        ])
        .unwrap_err(),
        DefinitionError::InputOverflow(material("wood"))
    );
    assert_eq!(
        try_definition(vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(material("stone"), u32::MAX), (material("stone"), 1)],
        }])
        .unwrap_err(),
        DefinitionError::OutputOverflow(material("stone"))
    );
    assert!(
        try_definition(vec![BuildingBehavior::Production {
            inputs: vec![(material("wood"), u32::MAX)],
            outputs: vec![(material("stone"), u32::MAX)],
        }])
        .is_ok()
    );
}

#[test]
fn rejected_replacement_cannot_change_catalog_or_existing_instances() {
    let mut catalog = BuildingCatalog::default();
    catalog.register(definition(vec![], vec![(material("grain"), 1)]));
    let blueprint = catalog.get_blueprint("mill").unwrap();
    let building = Building::new(BuildingId(1), Position { x: 0, y: 0 }, blueprint.clone());
    let replacement = try_definition(vec![BuildingBehavior::Production {
        inputs: vec![],
        outputs: vec![(material("grain"), u32::MAX), (material("grain"), 1)],
    }]);
    assert_eq!(
        replacement.unwrap_err(),
        DefinitionError::OutputOverflow(material("grain"))
    );
    assert!(Arc::ptr_eq(
        &catalog.get_blueprint("mill").unwrap(),
        &blueprint
    ));
    assert!(std::ptr::eq(building.definition(), blueprint.as_ref()));
}

#[test]
fn invalid_efficiencies_do_not_change_existing_target() {
    let mut territory = TerritoryManager::default();
    let mut settlement = Settlement::new(SettlementId(1), 10, 1, Arc::default());
    let mut building = Building::new(
        BuildingId(1),
        Position { x: 0, y: 0 },
        Arc::new(definition(vec![], vec![(material("grain"), 1)])),
    );
    building.set_target_load(0.5).unwrap();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
        assert!(building.set_target_load(value).is_err());
        assert_eq!(building.target_load(), 0.5);
    }
    settlement
        .add_building(
            building,
            &mut MapSettlementEnvironment::new(&test_map(), &mut test_finder(), &mut territory),
        )
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
fn blueprint_owns_its_recipe_and_exposes_only_shared_views() {
    let mut behaviors = vec![BuildingBehavior::Production {
        inputs: vec![(material("wood"), 1)],
        outputs: vec![(material("stone"), 2)],
    }];
    let definition = BuildingDefinition::new(
        "mill",
        "Mill",
        2,
        3,
        behaviors.clone(),
        ProjectCost {
            turn_cost: 4,
            ..ProjectCost::default()
        },
    )
    .unwrap();
    behaviors.clear();
    assert_eq!(definition.id(), "mill");
    assert_eq!(definition.name(), "Mill");
    assert_eq!(definition.tier(), 2);
    assert_eq!(definition.max_workers(), 3);
    assert_eq!(definition.behaviors().len(), 1);
    assert_eq!(definition.construction_cost().turn_cost, 4);
}

#[test]
fn variable_length_storage_order_is_validated_atomically() {
    let mut settlement = Settlement::new(SettlementId(1), 10, 1, Arc::default());
    let mut order = ResourceCatalog::standard().materials().to_vec();
    order[..3].rotate_right(1);
    settlement.set_storage_order(&order).unwrap();
    assert_eq!(
        settlement.set_storage_order(vec![material("wood"), material("stone")]),
        Err(StorageOrderError::MissingMaterial(material("grain")))
    );
    assert_eq!(
        settlement.set_storage_order(Vec::<ResourceType>::new()),
        Err(StorageOrderError::MissingMaterial(material("wood")))
    );
    assert_eq!(
        settlement.set_storage_order([material("wood"), material("stone"), material("grain")]),
        Err(StorageOrderError::MissingMaterial(material("cattle")))
    );
    assert_eq!(settlement.storage_order(), order);
    assert_eq!(
        settlement.storage_order().len(),
        ResourceCatalog::standard().materials().len()
    );
}

#[test]
fn rejects_invalid_staffing_and_labour_recipes_at_creation() {
    assert_eq!(
        try_definition(vec![BuildingBehavior::StatModifier {
            stat: simulation_core::buildings::effects::StatType::HousingCapacity,
            base_amount: u32::MAX,
            per_worker_amount: 1,
        }])
        .unwrap_err(),
        DefinitionError::StatOverflow
    );
    let staffed = |max_workers| {
        BuildingDefinition::new("big", "Big", 1, max_workers, vec![], Default::default())
    };
    assert!(staffed(BuildingDefinition::MAX_WORKERS).is_ok());
    assert_eq!(
        staffed(BuildingDefinition::MAX_WORKERS + 1).unwrap_err(),
        DefinitionError::TooManyWorkers
    );
    assert_eq!(
        try_definition(vec![BuildingBehavior::Production {
            inputs: vec![(ResourceType::Labour, 1)],
            outputs: vec![],
        }])
        .unwrap_err(),
        DefinitionError::LabourInRecipe
    );
    assert_eq!(
        try_definition(vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(ResourceType::Labour, 1)],
        }])
        .unwrap_err(),
        DefinitionError::LabourInRecipe
    );
}
