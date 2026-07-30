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

    /// 1. Faza Demand: Zbieranie zapotrzebowania od trwających i nie zapauzowanych projektów
    pub fn generate_requests(&self) -> Vec<DemandRequest> {
        self.active_projects
            .iter()
            .filter_map(|project| {
                let request = project.as_demand_request();
                // Wysyłamy żądanie tylko jeśli projekt faktycznie czegoś potrzebuje
                if request.demand.is_empty() {
                    None
                } else {
                    Some(request)
                }
            })
            .collect()
    }

    /// 2. Faza Postępu: Aplikowanie przydzielonych zasobów
    pub fn process_allocation(&mut self, allocations: &AllocationResult) {
        for project in &mut self.active_projects {
            let grants = allocations.get(&project.id);
            project.resolve_allocation(grants);
        }
    }

    /// 3. Faza Finalizacji: Wyciąganie ukończonych projektów z kolejki
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