use core::domain::buildings::core::{Building, BuildingLocation, Position};
use core::domain::buildings::factory::{BuildingBehavior, BuildingDefinition};
use core::domain::economy::resource::ResourceType::{Grain, Wood};
use core::domain::ids::{BuildingId, ProjectId};
use core::domain::settlement::core::BuildingCommandError;
use core::domain::settlement::core::Settlement;
use core::domain::settlement::projects::{ProjectCompletion, ProjectCost, ProjectError};
use std::sync::Arc;

fn blueprint() -> Arc<BuildingDefinition> {
    Arc::new(BuildingDefinition {
        id: "farm".into(),
        name: "Farm".into(),
        tier: 1,
        max_workers: 1,
        behaviors: vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(Grain, 1)],
        }],
        construction_cost: ProjectCost::default(),
    })
}

#[test]
fn settlement_commands_preserve_capacity_and_stock() {
    let mut settlement = Settlement::new(7, 10, 1, Arc::default());
    {
        assert_eq!(settlement.id(), 7);
        assert_eq!(settlement.population(), 1);
        assert_eq!(settlement.deposit(Wood, 20).accepted, 10);
        assert!(settlement.set_inventory_capacity(5).is_err());
        assert!(!settlement.withdraw(Wood, 11));
        assert_eq!(settlement.inventory().amount(Wood), 10);
        assert!(settlement.withdraw(Wood, 5));
        settlement.set_inventory_capacity(5).unwrap();
        assert_eq!(settlement.inventory().capacity(), 5);
    }
    assert_eq!(settlement.inventory().amount(Wood), 5);
}

#[test]
fn building_commands_and_projects_reach_the_turn_engine() {
    let mut settlement = Settlement::new(1, 10, 1, Arc::default());
    settlement
        .add_building(
            Building::new(
                BuildingId(5),
                Position { x: 0, y: 0 },
                BuildingLocation::Hub,
                blueprint(),
            )
            .unwrap(),
        )
        .unwrap();
    settlement
        .set_building_efficiency(BuildingId(5), 1.0)
        .unwrap();
    settlement
        .set_building_active(BuildingId(5), false)
        .unwrap();
    assert_eq!(
        settlement.set_building_active(BuildingId(9), true),
        Err(BuildingCommandError::MissingBuilding(BuildingId(9)))
    );
    assert!(settlement.process_turn().inventory.is_balanced());
    assert_eq!(settlement.inventory().amount(Grain), 0);
    settlement.set_building_active(BuildingId(5), true).unwrap();
    settlement
        .queue_construction(
            ProjectId(1),
            BuildingLocation::Spoke,
            Position { x: 1, y: 0 },
            blueprint(),
            ProjectCost::default(),
        )
        .unwrap();
    assert_eq!(
        settlement.queue_construction(
            ProjectId(1),
            BuildingLocation::Spoke,
            Position { x: 1, y: 0 },
            blueprint(),
            ProjectCost::default()
        ),
        Err(ProjectError::DuplicateProject(ProjectId(1)))
    );
    let report = settlement.process_turn();
    assert!(report.inventory.is_balanced());
    assert!(matches!(
        report.projects[0].result,
        Ok(ProjectCompletion::BuildingConstructed(_))
    ));
    assert_eq!(settlement.inventory().amount(Grain), 1);
    assert_eq!(settlement.buildings().count(), 2);
    assert!(settlement.active_projects().is_empty());
}
