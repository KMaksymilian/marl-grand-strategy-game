use std::collections::HashMap;
use std::sync::Arc;
use crate::domain::buildings::core::{Building, BuildingLocation};
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::economy::resource::ResourceType;
use crate::domain::economy::resource_allocation::{AllocationResult, DemandRequest};

#[derive(Debug)]
pub struct BuildingManager {
    pub hub_buildings: Vec<Building>,
    pub spoke_buildings: Vec<Building>,
}

impl BuildingManager {
    pub fn new() -> Self {
        Self {
            hub_buildings: Vec::new(),
            spoke_buildings: Vec::new(),
        }
    }

    pub fn construct_building(&mut self, building: Building) {
        match building.location {
            BuildingLocation::Hub => self.hub_buildings.push(building),
            BuildingLocation::Spoke => self.spoke_buildings.push(building),
        }
    }

    pub fn upgrade_building(&mut self, target_id: u32, new_def: Arc<BuildingDefinition>) -> bool {
        if let Some(building) = self.hub_buildings.iter_mut()
            .chain(self.spoke_buildings.iter_mut())
            .find(|b| b.instance_id == target_id)
        {
            building.definition = new_def;
            true
        } else {
            false
        }
    }

    pub fn generate_requests(&self) -> Vec<DemandRequest> {
        self.hub_buildings.iter().chain(self.spoke_buildings.iter())
            .filter_map(|b| {
                let demand = b.calculate_demand();
                if demand.is_empty() {
                    None
                } else {
                    Some(DemandRequest { entity_id: b.instance_id, demand })
                }
            })
            .collect()
    }

    pub fn process_allocations(&mut self, allocations: &AllocationResult) -> (HashMap<ResourceType, u32>, HashMap<ResourceType, u32>) {
        let mut all_produced = HashMap::new();
        let mut all_returned = HashMap::new();

        for building in self.hub_buildings.iter_mut().chain(self.spoke_buildings.iter_mut()) {
            let grants = allocations.get(&building.instance_id);
            let update_result = building.resolve_allocation(grants);

            for (res, amount) in update_result.production.produced {
                *all_produced.entry(res).or_insert(0) += amount;
            }
            for (res, amount) in update_result.production.unused {
                if amount > 0 {
                    *all_returned.entry(res).or_insert(0) += amount;
                }
            }
        }

        (all_produced, all_returned)
    }
}