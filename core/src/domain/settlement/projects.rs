use crate::domain::buildings::core::{BuildingLocation, Position};
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::economy::resource::ResourceType;
use crate::domain::economy::resource_allocation::DemandRequest;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ProjectResult {
    ConstructBuilding {
        location: BuildingLocation,
        position: Position,
        definition: Arc<BuildingDefinition>,
    },
    UpgradeBuilding {
        target_instance_id: u32,
        new_definition: Arc<BuildingDefinition>,
    },
    RecruitUnit {
        // unit_definition: Arc<UnitDefinition>
    },
}

#[derive(Default, Debug, Clone)]
pub struct ProjectCost {
    pub needed_resources: HashMap<ResourceType, u32>,
    pub turn_cost: u32,
}

impl ProjectCost {
    pub fn is_fully_paid(&self) -> bool {
        self.needed_resources.is_empty() && self.turn_cost == 0
    }
}
#[derive(Debug, Clone)]
pub struct Project {
    pub id: u32,
    pub result: ProjectResult,
    pub remaining_cost: ProjectCost,
}

impl Project {
    pub fn new(id: u32, result: ProjectResult, initial_cost: ProjectCost) -> Self {
        Self {
            id,
            result,
            remaining_cost: initial_cost,
        }
    }

    /// Demand request
    pub fn as_demand_request(&self) -> DemandRequest {
        DemandRequest {
            entity_id: self.id,
            demand: self.remaining_cost.needed_resources.clone(),
        }
    }

    /// Resolve allocation
    pub fn resolve_allocation(&mut self, allocated: Option<&HashMap<ResourceType, u32>>) {
        if let Some(grants) = allocated {
            for (res, amount) in grants {
                if let Some(needed) = self.remaining_cost.needed_resources.get_mut(res) {
                    *needed = needed.saturating_sub(*amount);

                    if *needed == 0 {
                        self.remaining_cost.needed_resources.remove(res);
                    }
                }
            }
        }

        if self.remaining_cost.turn_cost > 0 {
            self.remaining_cost.turn_cost -= 1;
        }
    }

    /// Finalize project
    pub fn is_completed(&self) -> bool {
        self.remaining_cost.is_fully_paid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::resource::ResourceType;
    use std::collections::HashMap;

    // --- Helpers ---

    // Creates a simple dummy result to initialize projects quickly without
    // needing to mock BuildingDefinitions and locations.
    fn create_dummy_result() -> ProjectResult {
        ProjectResult::RecruitUnit {}
    }

    // --- ProjectCost Tests ---

    #[test]
    fn test_project_cost_is_fully_paid_when_empty_and_zero_turns() {
        // Arrange
        let cost = ProjectCost {
            needed_resources: HashMap::new(),
            turn_cost: 0,
        };

        // Act
        let result = cost.is_fully_paid();

        // Assert
        assert!(
            result,
            "Cost should be fully paid when there are no resource demands and zero turns left"
        );
    }

    #[test]
    fn test_project_cost_is_not_paid_when_resources_remain() {
        // Arrange
        let mut resources = HashMap::new();
        resources.insert(ResourceType::Wood, 10);

        let cost = ProjectCost {
            needed_resources: resources,
            turn_cost: 0,
        };

        // Act
        let result = cost.is_fully_paid();

        // Assert
        assert!(
            !result,
            "Cost should NOT be fully paid if resource demands are not empty"
        );
    }

    #[test]
    fn test_project_cost_is_not_paid_when_turns_remain() {
        // Arrange
        let cost = ProjectCost {
            needed_resources: HashMap::new(),
            turn_cost: 1, // 1 turn left
        };

        // Act
        let result = cost.is_fully_paid();

        // Assert
        assert!(
            !result,
            "Cost should NOT be fully paid if turn cost is greater than 0"
        );
    }

    // --- Project Tests ---

    #[test]
    fn test_project_initialization() {
        // Arrange
        let mut resources = HashMap::new();
        resources.insert(ResourceType::Wood, 50);
        let initial_cost = ProjectCost {
            needed_resources: resources,
            turn_cost: 3,
        };

        // Act
        let project = Project::new(1, create_dummy_result(), initial_cost);

        // Assert
        assert_eq!(project.id, 1);
        assert_eq!(project.remaining_cost.turn_cost, 3);
        assert_eq!(
            *project
                .remaining_cost
                .needed_resources
                .get(&ResourceType::Wood)
                .unwrap(),
            50
        );
    }

    #[test]
    fn test_as_demand_request_creates_correct_mapping() {
        // Arrange
        let mut resources = HashMap::new();
        resources.insert(ResourceType::Stone, 100);
        resources.insert(ResourceType::Wood, 50);

        let project = Project::new(
            10,
            create_dummy_result(),
            ProjectCost {
                needed_resources: resources,
                turn_cost: 1,
            },
        );

        // Act
        let request = project.as_demand_request();

        // Assert
        assert_eq!(
            request.entity_id, 10,
            "DemandRequest should inherit the project's ID"
        );
        assert_eq!(
            request.demand.len(),
            2,
            "DemandRequest should copy all needed resources"
        );
        assert_eq!(*request.demand.get(&ResourceType::Stone).unwrap(), 100);
    }

    #[test]
    fn test_resolve_allocation_deducts_resources_and_turns() {
        // Arrange
        let mut resources = HashMap::new();
        resources.insert(ResourceType::Wood, 50);

        let mut project = Project::new(
            1,
            create_dummy_result(),
            ProjectCost {
                needed_resources: resources,
                turn_cost: 5,
            },
        );

        let mut grants = HashMap::new();
        grants.insert(ResourceType::Wood, 20); // Partial allocation

        // Act
        project.resolve_allocation(Some(&grants));

        // Assert
        assert_eq!(
            *project
                .remaining_cost
                .needed_resources
                .get(&ResourceType::Wood)
                .unwrap(),
            30,
            "Wood need should be reduced from 50 to 30"
        );
        assert_eq!(
            project.remaining_cost.turn_cost, 4,
            "Turn cost should be decremented by 1"
        );
    }

    #[test]
    fn test_resolve_allocation_removes_fulfilled_resources() {
        // Arrange
        let mut resources = HashMap::new();
        resources.insert(ResourceType::Stone, 20);

        let mut project = Project::new(
            1,
            create_dummy_result(),
            ProjectCost {
                needed_resources: resources,
                turn_cost: 1,
            },
        );

        let mut grants = HashMap::new();
        grants.insert(ResourceType::Stone, 20); // Exact allocation

        // Act
        project.resolve_allocation(Some(&grants));

        // Assert
        assert!(
            project.remaining_cost.needed_resources.is_empty(),
            "Needed resources map should be empty when fully satisfied"
        );
    }

    #[test]
    fn test_resolve_allocation_handles_over_allocation_safely() {
        // Arrange
        let mut resources = HashMap::new();
        resources.insert(ResourceType::Wood, 10);

        let mut project = Project::new(
            1,
            create_dummy_result(),
            ProjectCost {
                needed_resources: resources,
                turn_cost: 1,
            },
        );

        let mut grants = HashMap::new();
        grants.insert(ResourceType::Wood, 50); // Granted way more than needed

        // Act
        project.resolve_allocation(Some(&grants));

        // Assert
        assert!(
            project.remaining_cost.needed_resources.is_empty(),
            "Needed resources should be fully cleared via saturating_sub without overflowing"
        );
    }

    #[test]
    fn test_resolve_allocation_handles_none_grants_by_only_decrementing_turn() {
        // Arrange
        let mut resources = HashMap::new();
        resources.insert(ResourceType::Wood, 50);

        let mut project = Project::new(
            1,
            create_dummy_result(),
            ProjectCost {
                needed_resources: resources,
                turn_cost: 3,
            },
        );

        // Act
        project.resolve_allocation(None);

        // Assert
        assert_eq!(
            *project
                .remaining_cost
                .needed_resources
                .get(&ResourceType::Wood)
                .unwrap(),
            50,
            "Resources should remain untouched when no grants are provided"
        );
        assert_eq!(
            project.remaining_cost.turn_cost, 2,
            "Turn cost should still be decremented even if no resources are granted"
        );
    }

    #[test]
    fn test_resolve_allocation_does_not_underflow_turn_cost() {
        // Arrange
        let mut project = Project::new(
            1,
            create_dummy_result(),
            ProjectCost {
                needed_resources: HashMap::new(),
                turn_cost: 0,
            },
        );

        // Act
        project.resolve_allocation(None);

        // Assert
        assert_eq!(
            project.remaining_cost.turn_cost, 0,
            "Turn cost should not drop below zero"
        );
    }

    #[test]
    fn test_is_completed_checks_both_conditions() {
        // Arrange
        let mut resources = HashMap::new();
        resources.insert(ResourceType::Wood, 10);

        let mut project = Project::new(
            1,
            create_dummy_result(),
            ProjectCost {
                needed_resources: resources,
                turn_cost: 1,
            },
        );

        // Act & Assert 1: Has resources, has turns
        assert!(!project.is_completed());

        // Act & Assert 2: Has no resources, has turns
        let mut exact_grants = HashMap::new();
        exact_grants.insert(ResourceType::Wood, 10);
        project.resolve_allocation(Some(&exact_grants)); // This drops turns to 0 and clears resources
        assert!(
            project.is_completed(),
            "Project should be completed once turns hit 0 and resources are clear"
        );
    }
}
