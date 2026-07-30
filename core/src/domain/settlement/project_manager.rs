use crate::domain::economy::resource_allocation::{AllocationResult, DemandRequest};
use crate::domain::settlement::projects::Project;

#[derive(Debug)]
pub struct ProjectManager {
    pub active_projects: Vec<Project>,
}

impl ProjectManager {
    pub fn new() -> Self {
        Self {
            active_projects: Vec::new(),
        }
    }

    pub fn add_project(&mut self, project: Project) {
        self.active_projects.push(project);
    }

    /// 1.Demand Phase
    pub fn generate_requests(&self) -> Vec<DemandRequest> {
        self.active_projects
            .iter()
            .filter_map(|project| {
                let request = project.as_demand_request();
                if request.demand.is_empty() {
                    None
                } else {
                    Some(request)
                }
            })
            .collect()
    }

    /// 2. Progress phase
    pub fn process_allocation(&mut self, allocations: &AllocationResult) {
        for project in &mut self.active_projects {
            let grants = allocations.get(&project.id);
            project.resolve_allocation(grants);
        }
    }

    /// 3. Finalization phase
    pub fn extract_completed(&mut self) -> Vec<Project> {
        let mut completed = Vec::new();
        let mut i = 0;

        while i < self.active_projects.len() {
            if self.active_projects[i].is_completed() {
                completed.push(self.active_projects.remove(i));
            } else {
                i += 1;
            }
        }

        completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::resource::ResourceType;
    use crate::domain::economy::resource_allocation::AllocationResult;
    use crate::domain::settlement::projects::{Project, ProjectCost, ProjectResult};
    use std::collections::HashMap;

    // --- Helpers ---

    fn create_dummy_result() -> ProjectResult {
        ProjectResult::RecruitUnit {}
    }

    fn create_test_project(id: u32, wood_cost: u32, turn_cost: u32) -> Project {
        let mut resources = HashMap::new();
        if wood_cost > 0 {
            resources.insert(ResourceType::Wood, wood_cost);
        }

        Project::new(
            id,
            create_dummy_result(),
            ProjectCost {
                needed_resources: resources,
                turn_cost,
            },
        )
    }

    // --- Tests ---

    #[test]
    fn test_add_project_increases_active_count() {
        // Arrange
        let mut manager = ProjectManager::new();
        let project = create_test_project(1, 100, 5);

        // Act
        manager.add_project(project);

        // Assert
        assert_eq!(
            manager.active_projects.len(),
            1,
            "Project should be added to the active list"
        );
        assert_eq!(
            manager.active_projects[0].id, 1,
            "The added project should retain its ID"
        );
    }

    #[test]
    fn test_generate_requests_filters_empty_demands() {
        // Arrange
        let mut manager = ProjectManager::new();

        // Project 1 needs 50 Wood
        manager.add_project(create_test_project(1, 50, 2));
        // Project 2 needs NO resources (0 Wood), only 2 turns to finish
        manager.add_project(create_test_project(2, 0, 2));
        // Project 3 needs 10 Wood
        manager.add_project(create_test_project(3, 10, 1));

        // Act
        let requests = manager.generate_requests();

        // Assert
        assert_eq!(
            requests.len(),
            2,
            "Should only generate requests for projects that actually need resources"
        );

        let request_ids: Vec<u32> = requests.iter().map(|req| req.entity_id).collect();
        assert!(
            request_ids.contains(&1),
            "Project 1 should generate a request"
        );
        assert!(
            !request_ids.contains(&2),
            "Project 2 should NOT generate a request"
        );
        assert!(
            request_ids.contains(&3),
            "Project 3 should generate a request"
        );
    }

    #[test]
    fn test_process_allocation_routes_grants_to_correct_projects() {
        // Arrange
        let mut manager = ProjectManager::new();

        // P1 needs 100 Wood, P2 needs 50 Wood
        manager.add_project(create_test_project(1, 100, 2));
        manager.add_project(create_test_project(2, 50, 2));

        let mut allocations: AllocationResult = HashMap::new();

        // Grant 40 Wood to Project 1
        let mut p1_grants = HashMap::new();
        p1_grants.insert(ResourceType::Wood, 40);
        allocations.insert(1, p1_grants);

        // Act
        manager.process_allocation(&allocations);

        // Assert
        // Project 1 should have its wood need reduced from 100 to 60, and turns reduced to 1
        assert_eq!(
            *manager.active_projects[0]
                .remaining_cost
                .needed_resources
                .get(&ResourceType::Wood)
                .unwrap(),
            60
        );
        assert_eq!(manager.active_projects[0].remaining_cost.turn_cost, 1);

        // Project 2 received no grants, but its turn counter should still drop by 1
        assert_eq!(
            *manager.active_projects[1]
                .remaining_cost
                .needed_resources
                .get(&ResourceType::Wood)
                .unwrap(),
            50
        );
        assert_eq!(manager.active_projects[1].remaining_cost.turn_cost, 1);
    }

    #[test]
    fn test_extract_completed_removes_finished_and_keeps_pending() {
        // Arrange
        let mut manager = ProjectManager::new();

        // Setup 3 projects
        manager.add_project(create_test_project(1, 10, 1)); // Needs 10 Wood, 1 Turn
        manager.add_project(create_test_project(2, 50, 5)); // Needs 50 Wood, 5 Turns
        manager.add_project(create_test_project(3, 0, 1)); // Needs 0 Wood, 1 Turn

        // We simulate a turn passing where Project 1 gets what it needs,
        // Project 2 gets nothing, and Project 3 (which only needs a turn) advances.
        let mut allocations: AllocationResult = HashMap::new();
        let mut p1_grants = HashMap::new();
        p1_grants.insert(ResourceType::Wood, 10);
        allocations.insert(1, p1_grants);

        manager.process_allocation(&allocations);

        // Act
        let completed_projects = manager.extract_completed();

        // Assert
        // P1 is done (got 10 wood, turn dropped to 0)
        // P3 is done (needed 0 wood, turn dropped to 0)
        // P2 is NOT done (turn dropped to 4, still needs 50 wood)
        assert_eq!(
            completed_projects.len(),
            2,
            "Should extract exactly two completed projects"
        );
        assert_eq!(
            manager.active_projects.len(),
            1,
            "Should leave exactly one active project behind"
        );

        // Verify correct projects were extracted
        let completed_ids: Vec<u32> = completed_projects.iter().map(|p| p.id).collect();
        assert!(
            completed_ids.contains(&1),
            "Completed project 1 should be extracted"
        );
        assert!(
            completed_ids.contains(&3),
            "Completed project 3 should be extracted"
        );

        // Verify correct project remains active
        assert_eq!(
            manager.active_projects[0].id, 2,
            "Incomplete project 2 should remain active"
        );
    }
}
