use std::collections::HashMap;
use std::sync::Arc;
use crate::domain::buildings::core::{BuildingLocation, Position};
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::economy::resource::ResourceType;
use crate::domain::economy::resource_allocation::DemandRequest;

#[derive(Debug, Clone)]
pub enum ProjectResult {
    ConstructBuilding {
        location: BuildingLocation,
        position: Position, // <- Dodana pozycja
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
pub struct ProjectCost{
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