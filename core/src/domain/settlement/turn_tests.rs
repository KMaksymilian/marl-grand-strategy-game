use super::*;
use crate::domain::buildings::factory::BuildingBehavior;
use crate::domain::economy::resource::ResourceType;
use crate::domain::settlement::demographics::NeedTier;
use std::collections::HashMap;

fn definition(behaviors: Vec<BuildingBehavior>) -> Arc<BuildingDefinition> {
    Arc::new(BuildingDefinition {
        id: "test".into(),
        name: "Test".into(),
        tier: 1,
        max_workers: 1,
        behaviors,
        construction_cost: ProjectCost::default(),
    })
}

fn recipe(input: ResourceType, output: ResourceType) -> BuildingBehavior {
    BuildingBehavior::Production {
        inputs: vec![(input, 1)],
        outputs: vec![(output, 1)],
    }
}

fn add_building(settlement: &mut Settlement, id: u32, definition: Arc<BuildingDefinition>) {
    let mut building = Building::new(
        BuildingId(id),
        Position { x: 0, y: 0 },
        BuildingLocation::Hub,
        definition,
    );
    building.target_efficiency = 1.0;
    settlement
        .building_manager
        .construct_building(building)
        .unwrap();
}

fn stock(settlement: &Settlement, resource: ResourceType) -> u32 {
    settlement
        .inventory_manager
        .main
        .resources
        .get(&resource)
        .copied()
        .unwrap_or(0)
}

#[test]
fn building_and_project_with_same_number_have_separate_allocations() {
    let mut settlement = Settlement::new(1, 100, 1, Arc::default());
    let blueprint = definition(vec![recipe(ResourceType::Wood, ResourceType::Stone)]);
    add_building(&mut settlement, 7, blueprint.clone());
    settlement
        .queue_construction(
            ProjectId(7),
            BuildingLocation::Hub,
            Position { x: 1, y: 0 },
            blueprint,
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                turn_cost: 1,
            },
        )
        .unwrap();
    settlement.inventory_manager.main.add(ResourceType::Wood, 2);

    settlement.process_turn();

    assert_eq!(stock(&settlement, ResourceType::Wood), 0);
    assert_eq!(stock(&settlement, ResourceType::Stone), 1);
    assert!(settlement.project_manager.active_projects.is_empty());
    let buildings = &settlement.building_manager.hub_buildings;
    assert_eq!(buildings.len(), 2);
    assert_ne!(buildings[0].instance_id, buildings[1].instance_id);
}

#[test]
fn population_and_building_zero_have_separate_allocations() {
    let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
        min_population: 1,
        needs_per_capita: HashMap::from([(ResourceType::Grain, 1.0)]),
    }]));
    let mut settlement = Settlement::new(1, 100, 1, registry);
    add_building(
        &mut settlement,
        0,
        definition(vec![recipe(ResourceType::Grain, ResourceType::Stone)]),
    );
    settlement
        .inventory_manager
        .main
        .add(ResourceType::Grain, 2);

    let consumption = settlement.process_turn();

    assert_eq!(consumption.satisfaction_percent, 1.0);
    assert_eq!(stock(&settlement, ResourceType::Grain), 0);
    assert_eq!(stock(&settlement, ResourceType::Stone), 1);
}

#[test]
fn shared_recipe_inputs_are_consumed_only_once() {
    let mut settlement = Settlement::new(1, 100, 1, Arc::default());
    add_building(
        &mut settlement,
        1,
        definition(vec![
            recipe(ResourceType::Wood, ResourceType::Stone),
            recipe(ResourceType::Wood, ResourceType::Grain),
        ]),
    );
    settlement.inventory_manager.main.add(ResourceType::Wood, 1);

    settlement.process_turn();

    assert_eq!(stock(&settlement, ResourceType::Wood), 0);
    assert_eq!(stock(&settlement, ResourceType::Stone), 1);
    assert_eq!(stock(&settlement, ResourceType::Grain), 0);
}

#[test]
fn remaining_single_material_advances_one_of_two_projects() {
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    for id in [1, 2] {
        settlement
            .queue_construction(
                ProjectId(id),
                BuildingLocation::Hub,
                Position { x: 0, y: 0 },
                definition(vec![]),
                ProjectCost {
                    needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                    turn_cost: 1,
                },
            )
            .unwrap();
    }
    settlement.inventory_manager.main.add(ResourceType::Wood, 1);

    settlement.process_turn();

    assert_eq!(stock(&settlement, ResourceType::Wood), 0);
    assert_eq!(settlement.building_manager.hub_buildings.len(), 1);
    assert_eq!(settlement.project_manager.active_projects.len(), 1);
    assert_eq!(
        settlement.project_manager.active_projects[0].id,
        ProjectId(2)
    );
}

#[test]
fn large_project_cost_is_deducted_exactly() {
    let amount = 16_777_219;
    let mut settlement = Settlement::new(1, u32::MAX, 0, Arc::default());
    settlement
        .queue_construction(
            ProjectId(1),
            BuildingLocation::Hub,
            Position { x: 0, y: 0 },
            definition(vec![]),
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, amount)]),
                turn_cost: 1,
            },
        )
        .unwrap();
    settlement
        .inventory_manager
        .main
        .add(ResourceType::Wood, amount);

    settlement.process_turn();

    assert_eq!(stock(&settlement, ResourceType::Wood), 0);
    assert_eq!(settlement.building_manager.hub_buildings.len(), 1);
}

#[test]
fn production_is_available_for_consumption_in_the_next_turn() {
    let mut settlement = Settlement::new(1, 100, 2, Arc::default());
    add_building(
        &mut settlement,
        1,
        definition(vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(ResourceType::Grain, 1)],
        }]),
    );
    add_building(
        &mut settlement,
        2,
        definition(vec![recipe(ResourceType::Grain, ResourceType::Stone)]),
    );

    settlement.process_turn();
    assert_eq!(stock(&settlement, ResourceType::Grain), 1);
    assert_eq!(stock(&settlement, ResourceType::Stone), 0);

    settlement.process_turn();
    assert_eq!(stock(&settlement, ResourceType::Grain), 1);
    assert_eq!(stock(&settlement, ResourceType::Stone), 1);
    assert!(
        settlement
            .inventory_manager
            .pending_production
            .resources
            .is_empty()
    );
}
