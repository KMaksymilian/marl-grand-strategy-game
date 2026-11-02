//! Domain integration without a map, terrain generator or pathfinding buffers.
use std::sync::Arc;

use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::buildings::productivity::{
    BuildingProductivityCalculator, TravelCostEfficiencyPolicy, TravelCostSource,
};
use simulation_core::economy::{ResourceType, cost::ProjectCost};
use simulation_core::ids::{ProjectId, SettlementId};
use simulation_core::settlement::{Settlement, projects::ProjectCompletion};
use simulation_core::spatial::{
    BuildingReference, ConstructionSpace, Position, ProjectReference, TerritoryError, TileOccupancy,
};

const SITE: Position = Position { x: 1, y: 0 };

// One controlled site and a mutable directed cost, sufficient for these scenarios.
struct TestEnvironment {
    occupancy: TileOccupancy,
    cost: Option<u32>,
}

impl TravelCostSource for TestEnvironment {
    fn travel_cost(&mut self, from: Position, to: Position) -> Option<u32> {
        assert_eq!(from, Position { x: 0, y: 0 });
        assert_eq!(to, SITE);
        self.cost
    }
}

impl ConstructionSpace for TestEnvironment {
    fn reserve_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError> {
        assert_eq!(position, SITE);
        if self.occupancy != TileOccupancy::Empty {
            return Err(TerritoryError::Occupied {
                position,
                occupancy: self.occupancy,
            });
        }
        self.occupancy = TileOccupancy::Construction(project);
        Ok(())
    }

    fn validate_construction(
        &self,
        position: Position,
        expected: ProjectReference,
    ) -> Result<(), TerritoryError> {
        assert_eq!(position, SITE);
        if self.occupancy != TileOccupancy::Construction(expected) {
            return Err(TerritoryError::ReservationMismatch {
                position,
                expected,
                actual: self.occupancy,
            });
        }
        Ok(())
    }

    fn cancel_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
    ) -> Result<(), TerritoryError> {
        self.validate_construction(position, project)?;
        self.occupancy = TileOccupancy::Empty;
        Ok(())
    }

    fn finish_construction(
        &mut self,
        position: Position,
        project: ProjectReference,
        building: BuildingReference,
    ) -> Result<(), TerritoryError> {
        self.validate_construction(position, project)?;
        assert_eq!(project.settlement_id, building.settlement_id);
        self.occupancy = TileOccupancy::Building(building);
        Ok(())
    }

    fn register_building(
        &mut self,
        position: Position,
        building: BuildingReference,
    ) -> Result<(), TerritoryError> {
        assert_eq!(position, SITE);
        if self.occupancy != TileOccupancy::Empty {
            return Err(TerritoryError::Occupied {
                position,
                occupancy: self.occupancy,
            });
        }
        self.occupancy = TileOccupancy::Building(building);
        Ok(())
    }

    fn release_rejected_construction(&mut self, position: Position, project: ProjectReference) {
        let _ = self.cancel_construction(position, project);
    }
}

fn workshop() -> Arc<BuildingDefinition> {
    Arc::new(
        BuildingDefinition::new(
            "workshop",
            "Workshop",
            1,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![(ResourceType::Wood, 2)],
                outputs: vec![(ResourceType::Stone, 10)],
            }],
            ProjectCost::default(),
        )
        .unwrap(),
    )
}

#[test]
fn construction_and_economy_accept_an_environment_without_a_map() {
    let mut environment = TestEnvironment {
        occupancy: TileOccupancy::Empty,
        cost: Some(200),
    };
    let mut settlement = Settlement::new(SettlementId(9), 100, 1, Arc::default());
    settlement.set_building_productivity_calculator(
        Some(BuildingProductivityCalculator::new(
            Position { x: 0, y: 0 },
            TravelCostEfficiencyPolicy::new(100, 300).unwrap(),
        )),
        &mut environment,
    );
    settlement
        .queue_construction(
            ProjectId(3),
            SITE,
            workshop(),
            ProjectCost::default(),
            &mut environment,
        )
        .unwrap();
    let report = settlement.process_turn(&mut environment, &mut []);
    let building = match report.projects[0].result.as_ref().unwrap() {
        ProjectCompletion::BuildingConstructed(id) => *id,
        other => panic!("unexpected completion: {other:?}"),
    };
    assert_eq!(
        environment.occupancy,
        TileOccupancy::Building(BuildingReference {
            settlement_id: settlement.id(),
            building_id: building
        })
    );
    settlement.set_building_load(building, 1.0).unwrap();
    settlement.deposit(ResourceType::Wood, 10);
    let report = settlement.process_turn(&mut environment, &mut []);
    assert_eq!(report.inventory.produced[&ResourceType::Stone], 5);
    assert_eq!(settlement.inventory().amount(ResourceType::Wood), 8);
    assert!(report.inventory.is_balanced());
    environment.cost = None;
    let report = settlement.process_turn(&mut environment, &mut []);
    assert_eq!(
        report
            .inventory
            .produced
            .get(&ResourceType::Stone)
            .copied()
            .unwrap_or(0),
        0
    );
    assert!(report.inventory.is_balanced());
}

#[test]
fn cancellation_uses_qualified_identity_and_preserves_foreign_reservations() {
    let mut environment = TestEnvironment {
        occupancy: TileOccupancy::Empty,
        cost: None,
    };
    let mut settlement = Settlement::new(SettlementId(9), 100, 0, Arc::default());
    settlement
        .queue_construction(
            ProjectId(3),
            SITE,
            workshop(),
            ProjectCost::default(),
            &mut environment,
        )
        .unwrap();
    let reservation = environment.occupancy;
    environment.occupancy = TileOccupancy::Construction(ProjectReference {
        settlement_id: SettlementId(10),
        project_id: ProjectId(3),
    });
    assert!(
        settlement
            .cancel_project(ProjectId(3), &mut environment)
            .is_err()
    );
    assert_eq!(settlement.active_projects().len(), 1);
    environment.occupancy = reservation;
    settlement
        .cancel_project(ProjectId(3), &mut environment)
        .unwrap();
    assert_eq!(environment.occupancy, TileOccupancy::Empty);
    assert!(settlement.active_projects().is_empty());
}
