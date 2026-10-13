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
fn zero_cost_entries_do_not_block_construction() {
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    settlement
        .queue_construction(
            ProjectId(10),
            BuildingLocation::Hub,
            Position { x: 0, y: 0 },
            definition(vec![]),
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 0)]),
                turn_cost: 0,
            },
        )
        .unwrap();
    let report = settlement.process_turn();
    assert!(settlement.project_manager.active_projects.is_empty());
    assert_eq!(settlement.building_manager.hub_buildings.len(), 1);
    assert_eq!(
        report.projects,
        vec![ProjectOutcome {
            project_id: ProjectId(10),
            result: Ok(ProjectCompletion::BuildingConstructed(BuildingId(0))),
        }]
    );
    assert!(settlement.process_turn().projects.is_empty());
}

#[test]
fn missing_upgrade_target_is_rejected_without_spending_resources() {
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    settlement
        .inventory_manager
        .main
        .add(ResourceType::Wood, 10);
    let result = settlement.queue_upgrade(
        ProjectId(10),
        BuildingId(99),
        definition(vec![]),
        ProjectCost {
            needed_resources: HashMap::from([(ResourceType::Wood, 5)]),
            turn_cost: 1,
        },
    );
    assert_eq!(result, Err(ProjectError::MissingBuilding(BuildingId(99))));
    assert!(settlement.project_manager.active_projects.is_empty());
    settlement.process_turn();
    assert_eq!(stock(&settlement, ResourceType::Wood), 10);
}

#[test]
fn upgrade_completion_reports_the_existing_building_id() {
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    let old = definition(vec![]);
    let upgraded = definition(vec![]);
    settlement
        .building_manager
        .construct_building(Building::new(
            BuildingId(8),
            Position { x: 2, y: 3 },
            BuildingLocation::Spoke,
            old,
        ))
        .unwrap();
    settlement
        .queue_upgrade(
            ProjectId(4),
            BuildingId(8),
            upgraded.clone(),
            ProjectCost {
                needed_resources: HashMap::from([
                    (ResourceType::Wood, 2),
                    (ResourceType::Stone, 0),
                ]),
                turn_cost: 1,
            },
        )
        .unwrap();
    settlement.inventory_manager.main.add(ResourceType::Wood, 2);

    let report = settlement.process_turn();

    assert_eq!(
        report.projects,
        vec![ProjectOutcome {
            project_id: ProjectId(4),
            result: Ok(ProjectCompletion::BuildingUpgraded(BuildingId(8))),
        }]
    );
    let building = &settlement.building_manager.spoke_buildings[0];
    assert_eq!(building.instance_id, BuildingId(8));
    assert_eq!(building.position, Position { x: 2, y: 3 });
    assert!(Arc::ptr_eq(&building.definition, &upgraded));
    assert_eq!(stock(&settlement, ResourceType::Wood), 0);
    assert!(settlement.project_manager.active_projects.is_empty());
    assert!(settlement.process_turn().projects.is_empty());
}

#[test]
fn removed_upgrade_target_fails_before_further_material_allocation() {
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    add_building(&mut settlement, 1, definition(vec![]));
    settlement
        .queue_upgrade(
            ProjectId(5),
            BuildingId(1),
            definition(vec![]),
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 3)]),
                turn_cost: 2,
            },
        )
        .unwrap();
    settlement.inventory_manager.main.add(ResourceType::Wood, 1);
    assert!(settlement.process_turn().projects.is_empty());
    assert_eq!(
        settlement.project_manager.active_projects[0]
            .remaining_cost
            .needed_resources[&ResourceType::Wood],
        2
    );

    settlement.building_manager.hub_buildings.clear();
    settlement.inventory_manager.main.add(ResourceType::Wood, 2);
    let report = settlement.process_turn();

    assert_eq!(
        report.projects,
        vec![ProjectOutcome {
            project_id: ProjectId(5),
            result: Err(ProjectError::MissingBuilding(BuildingId(1))),
        }]
    );
    assert_eq!(stock(&settlement, ResourceType::Wood), 2);
    assert!(settlement.project_manager.active_projects.is_empty());
    assert!(settlement.process_turn().projects.is_empty());
}

#[test]
fn unsupported_recruitment_reports_failure_without_spending() {
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    settlement
        .project_manager
        .add_project(Project::new(
            ProjectId(3),
            ProjectResult::RecruitUnit {},
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 2)]),
                turn_cost: 1,
            },
        ))
        .unwrap();
    settlement.inventory_manager.main.add(ResourceType::Wood, 2);
    let report = settlement.process_turn();
    assert_eq!(
        report.projects,
        vec![ProjectOutcome {
            project_id: ProjectId(3),
            result: Err(ProjectError::UnsupportedRecruitment),
        }]
    );
    assert_eq!(stock(&settlement, ResourceType::Wood), 2);
    assert!(settlement.project_manager.active_projects.is_empty());
}

#[test]
fn pending_construction_reports_completion_only_after_cost_and_time_are_paid() {
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    settlement
        .queue_construction(
            ProjectId(2),
            BuildingLocation::Hub,
            Position { x: 0, y: 0 },
            definition(vec![]),
            ProjectCost {
                needed_resources: HashMap::from([
                    (ResourceType::Wood, 2),
                    (ResourceType::Stone, 0),
                ]),
                turn_cost: 2,
            },
        )
        .unwrap();
    settlement.inventory_manager.main.add(ResourceType::Wood, 1);
    assert!(settlement.process_turn().projects.is_empty());
    assert_eq!(
        settlement.project_manager.active_projects[0]
            .remaining_cost
            .turn_cost,
        1
    );
    assert!(settlement.process_turn().projects.is_empty());
    assert_eq!(
        settlement.project_manager.active_projects[0]
            .remaining_cost
            .turn_cost,
        0
    );
    assert!(settlement.building_manager.hub_buildings.is_empty());

    settlement.inventory_manager.main.add(ResourceType::Wood, 1);
    let report = settlement.process_turn();
    assert_eq!(
        report.projects,
        vec![ProjectOutcome {
            project_id: ProjectId(2),
            result: Ok(ProjectCompletion::BuildingConstructed(BuildingId(0))),
        }]
    );
    assert_eq!(stock(&settlement, ResourceType::Wood), 0);
}

#[test]
fn duplicate_queue_requests_return_an_explicit_error() {
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    add_building(&mut settlement, 1, definition(vec![]));
    settlement
        .queue_construction(
            ProjectId(2),
            BuildingLocation::Hub,
            Position { x: 0, y: 0 },
            definition(vec![]),
            ProjectCost::default(),
        )
        .unwrap();
    assert_eq!(
        settlement.queue_upgrade(
            ProjectId(2),
            BuildingId(1),
            definition(vec![]),
            ProjectCost::default()
        ),
        Err(ProjectError::DuplicateProject(ProjectId(2)))
    );
    assert_eq!(settlement.project_manager.active_projects.len(), 1);
}

#[test]
fn finalization_checks_upgrade_targets_again() {
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    assert_eq!(
        settlement.finalize_project(ProjectResult::UpgradeBuilding {
            target_instance_id: BuildingId(42),
            new_definition: definition(vec![]),
        }),
        Err(ProjectError::MissingBuilding(BuildingId(42)))
    );
}

#[test]
fn invalid_projects_do_not_take_materials_from_valid_construction() {
    let mut settlement = Settlement::new(1, 100, 0, Arc::default());
    settlement
        .project_manager
        .add_project(Project::new(
            ProjectId(20),
            ProjectResult::UpgradeBuilding {
                target_instance_id: BuildingId(99),
                new_definition: definition(vec![]),
            },
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                turn_cost: 1,
            },
        ))
        .unwrap();
    settlement
        .queue_construction(
            ProjectId(21),
            BuildingLocation::Hub,
            Position { x: 0, y: 0 },
            definition(vec![]),
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                turn_cost: 1,
            },
        )
        .unwrap();
    settlement.inventory_manager.main.add(ResourceType::Wood, 1);

    let report = settlement.process_turn();

    assert_eq!(
        report.projects,
        vec![
            ProjectOutcome {
                project_id: ProjectId(20),
                result: Err(ProjectError::MissingBuilding(BuildingId(99)))
            },
            ProjectOutcome {
                project_id: ProjectId(21),
                result: Ok(ProjectCompletion::BuildingConstructed(BuildingId(0)))
            },
        ]
    );
    assert!(settlement.project_manager.active_projects.is_empty());
    assert_eq!(stock(&settlement, ResourceType::Wood), 0);
    assert_eq!(settlement.building_manager.hub_buildings.len(), 1);
}

#[test]
fn population_building_and_project_share_whole_resource_units() {
    let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
        min_population: 1,
        needs_per_capita: HashMap::from([(ResourceType::Wood, 1)]),
    }]));
    let mut settlement = Settlement::new(1, 100, 2, registry);
    let blueprint = definition(vec![recipe(ResourceType::Wood, ResourceType::Stone)]);
    add_building(&mut settlement, 1, blueprint.clone());
    settlement
        .queue_construction(
            ProjectId(1),
            BuildingLocation::Hub,
            Position { x: 1, y: 0 },
            blueprint,
            ProjectCost {
                needed_resources: HashMap::from([(ResourceType::Wood, 1)]),
                turn_cost: 1,
            },
        )
        .unwrap();
    settlement.inventory_manager.main.add(ResourceType::Wood, 4);

    let result = settlement.process_turn();

    // Two residents consume 2, the recipe consumes 1, construction costs 1.
    assert_eq!(stock(&settlement, ResourceType::Wood), 0);
    assert_eq!(stock(&settlement, ResourceType::Stone), 1);
    assert_eq!(result.consumption.satisfaction_percent, 1.0);
    assert!(result.consumption.missing_resources.is_empty());
    assert!(settlement.project_manager.active_projects.is_empty());
    assert_eq!(settlement.building_manager.hub_buildings.len(), 2);
}

#[test]
fn population_needs_use_the_same_units_as_inventory_in_a_full_turn() {
    let registry = Arc::new(NeedRegistry::new(vec![
        NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(ResourceType::Grain, 1)]),
        },
        NeedTier {
            min_population: 1,
            needs_per_capita: HashMap::from([(ResourceType::Grain, 1)]),
        },
    ]));
    let mut settlement = Settlement::new(1, 100, 1, registry);
    settlement
        .inventory_manager
        .main
        .add(ResourceType::Grain, 3);

    let result = settlement.process_turn();

    assert_eq!(result.consumption.satisfaction_percent, 1.0);
    assert!(result.consumption.missing_resources.is_empty());
    assert_eq!(stock(&settlement, ResourceType::Grain), 1);
}

#[test]
fn population_shortage_is_reported_across_turns() {
    let registry = Arc::new(NeedRegistry::new(vec![NeedTier {
        min_population: 1,
        needs_per_capita: HashMap::from([(ResourceType::Grain, 1), (ResourceType::Wood, 1)]),
    }]));
    let mut settlement = Settlement::new(1, 100, 1, registry);
    settlement
        .inventory_manager
        .main
        .add(ResourceType::Grain, 3);

    for remaining in [2, 1] {
        let result = settlement.process_turn();
        assert_eq!(result.consumption.satisfaction_percent, 0.5);
        assert_eq!(
            result.consumption.missing_resources,
            vec![ResourceType::Wood]
        );
        assert_eq!(stock(&settlement, ResourceType::Grain), remaining);
    }
    settlement.inventory_manager.main.add(ResourceType::Wood, 1);
    let result = settlement.process_turn();
    assert_eq!(result.consumption.satisfaction_percent, 1.0);
    assert!(result.consumption.missing_resources.is_empty());
    assert_eq!(stock(&settlement, ResourceType::Grain), 0);
    assert_eq!(stock(&settlement, ResourceType::Wood), 0);
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
        needs_per_capita: HashMap::from([(ResourceType::Grain, 1)]),
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

    let report = settlement.process_turn();

    assert_eq!(report.consumption.satisfaction_percent, 1.0);
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
