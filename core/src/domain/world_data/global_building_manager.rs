use crate::domain::buildings::core::{Building};
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::economy::resource::ResourceType;
use crate::domain::economy::resource_allocation::{AllocationResult, DemandRequest};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct BuildingManager {
    pub buildings: HashMap<u32, Vec<Building>>,
}

impl BuildingManager {
    pub fn new() -> Self {
        Self {
            buildings: Default::default(),
        }
    }

    pub fn construct_building(&mut self, settlement_id: u32, building: Building) {
        self.buildings.entry(settlement_id).or_default().push(building);
    }

    pub fn upgrade_building(&mut self, settlement_id: u32, target_id: u32, new_def: Arc<BuildingDefinition>) -> bool {
        if let Some(building) = self
            .buildings
            .entry(settlement_id)
            .or_default()
            .iter_mut()
            .find(|b| b.instance_id == target_id)
        {
            building.definition = new_def;
            true
        } else {
            false
        }
    }

    pub fn generate_requests(&self, settlement_id: u32) -> Vec<DemandRequest> {
        let empty = Vec::new();
        let buildings = self.buildings.get(&settlement_id).unwrap_or(&empty).iter();

        buildings
            .filter_map(|b| {
                let demand = b.calculate_demand();
                if demand.is_empty() {
                    None
                } else {
                    Some(DemandRequest {
                        entity_id: b.instance_id,
                        demand,
                    })
                }
            })
            .collect()
    }

    pub fn process_allocations(
        &mut self,
        settlement_id: u32,
        allocations: &AllocationResult,
    ) -> (HashMap<ResourceType, u32>, HashMap<ResourceType, u32>) {
        let mut all_produced = HashMap::new();
        let mut all_returned = HashMap::new();

        if let Some(buildings) = self.buildings.get_mut(&settlement_id) {
            for building in buildings.iter_mut() {
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
        }

        (all_produced, all_returned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::buildings::core::{Building, BuildingLocation};
    use crate::domain::buildings::factory::{BuildingBehavior, BuildingDefinition};
    use crate::domain::economy::resource::ResourceType;
    use crate::domain::world_data::point::Point;
    use std::collections::HashMap;
    use std::sync::Arc;

    // --- Helpers ---

    fn create_test_definition(
        id: &str,
        max_workers: u32,
        behaviors: Vec<BuildingBehavior>,
    ) -> Arc<BuildingDefinition> {
        Arc::new(BuildingDefinition {
            id: id.to_string(),
            name: "Test Building".to_string(),
            tier: 1,
            max_workers,
            behaviors,
            construction_cost: Default::default(),
        })
    }

    fn create_active_building(
        instance_id: u32,
        location: BuildingLocation,
        def: Arc<BuildingDefinition>,
    ) -> Building {
        // We pass 0 as settlement_id to the Building constructor
        let mut building = Building::new(instance_id, Point(0, 0), location, def, 0);
        building.target_efficiency = 1.0;
        building
    }

    // --- Tests ---

    #[test]
    fn test_construct_building_adds_to_settlement_list() {
        // Arrange
        let mut manager = BuildingManager::new();
        let def = create_test_definition("house", 0, vec![]);
        let settlement_id = 1;

        let hub_building = create_active_building(1, BuildingLocation::Hub, Arc::clone(&def));
        let spoke_building = create_active_building(2, BuildingLocation::Spoke, Arc::clone(&def));

        // Act
        manager.construct_building(settlement_id, hub_building);
        manager.construct_building(settlement_id, spoke_building);

        // Assert
        let buildings = manager.buildings.get(&settlement_id).expect("Settlement not found in HashMap");
        assert_eq!(
            buildings.len(),
            2,
            "Both buildings should be added to the same settlement ID"
        );
        assert_eq!(buildings[0].instance_id, 1);
        assert_eq!(buildings[1].instance_id, 2);
    }

    #[test]
    fn test_upgrade_building_success() {
        // Arrange
        let mut manager = BuildingManager::new();
        let old_def = create_test_definition("tier_1", 5, vec![]);
        let new_def = create_test_definition("tier_2", 10, vec![]);
        let settlement_id = 1;

        manager.construct_building(
            settlement_id,
            create_active_building(1, BuildingLocation::Hub, old_def),
        );

        // Act
        let result = manager.upgrade_building(settlement_id, 1, Arc::clone(&new_def));

        // Assert
        assert!(result, "Upgrade should return true for existing building");
        let buildings = manager.buildings.get(&settlement_id).unwrap();
        assert!(
            Arc::ptr_eq(&buildings[0].definition, &new_def),
            "Building definition should be successfully replaced"
        );
    }

    #[test]
    fn test_upgrade_building_not_found() {
        // Arrange
        let mut manager = BuildingManager::new();
        let def = create_test_definition("tier_1", 5, vec![]);
        let settlement_id = 1;

        manager.construct_building(
            settlement_id,
            create_active_building(1, BuildingLocation::Hub, def),
        );

        let new_def = create_test_definition("tier_2", 10, vec![]);

        // Act
        // Attempting to upgrade an instance ID that does not exist
        let result = manager.upgrade_building(settlement_id, 99, new_def);

        // Assert
        assert!(
            !result,
            "Upgrade should return false when targeting non-existent building ID"
        );
    }

    #[test]
    fn test_generate_requests_collects_from_all_active_buildings() {
        // Arrange
        let mut manager = BuildingManager::new();
        let settlement_id = 1;

        // Building 1: Hub, wants 5 Labour
        let def1 = create_test_definition("house", 5, vec![]);
        manager.construct_building(
            settlement_id,
            create_active_building(1, BuildingLocation::Hub, def1),
        );

        // Building 2: Spoke, wants 10 Labour + 10 Wood (via Production behavior)
        let prod_behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 10)],
            outputs: vec![(ResourceType::Stone, 20)],
        };
        let def2 = create_test_definition("lumber", 10, vec![prod_behavior]);
        manager.construct_building(
            settlement_id,
            create_active_building(2, BuildingLocation::Spoke, def2),
        );

        // Act
        let requests = manager.generate_requests(settlement_id);

        // Assert
        assert_eq!(requests.len(), 2, "Should generate exactly 2 demand requests");

        // Verify Building 1 demand
        let req1 = requests.iter().find(|r| r.entity_id == 1).unwrap();
        assert_eq!(*req1.demand.get(&ResourceType::Labour).unwrap(), 5);
        assert!(!req1.demand.contains_key(&ResourceType::Wood));

        // Verify Building 2 demand
        let req2 = requests.iter().find(|r| r.entity_id == 2).unwrap();
        assert_eq!(*req2.demand.get(&ResourceType::Labour).unwrap(), 10);
        assert_eq!(*req2.demand.get(&ResourceType::Wood).unwrap(), 10);
    }

    #[test]
    fn test_process_allocations_aggregates_production_and_unused() {
        // Arrange
        let mut manager = BuildingManager::new();
        let settlement_id = 1;

        let behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 10)],
            outputs: vec![(ResourceType::Stone, 20)],
        };

        let def = create_test_definition("stone_maker", 10, vec![behavior]);

        // Add two identical buildings
        manager.construct_building(
            settlement_id,
            create_active_building(1, BuildingLocation::Hub, Arc::clone(&def)),
        );
        manager.construct_building(
            settlement_id,
            create_active_building(2, BuildingLocation::Spoke, def),
        );

        // Setup AllocationResult
        let mut allocations: AllocationResult = HashMap::new();

        // Building 1 gets exactly what it needs
        let mut b1_grants = HashMap::new();
        b1_grants.insert(ResourceType::Labour, 10);
        b1_grants.insert(ResourceType::Wood, 10);
        allocations.insert(1, b1_grants);

        // Building 2 gets full labour, but EXTRA wood (15 instead of 10)
        let mut b2_grants = HashMap::new();
        b2_grants.insert(ResourceType::Labour, 10);
        b2_grants.insert(ResourceType::Wood, 15);
        allocations.insert(2, b2_grants);

        // Act
        let (produced, returned) = manager.process_allocations(settlement_id, &allocations);

        // Assert
        // Both buildings worked at 100% capacity (10 labour, enough wood).
        // Building 1 makes 20 stone, Building 2 makes 20 stone -> Total 40 stone.
        assert_eq!(
            *produced.get(&ResourceType::Stone).unwrap_or(&0),
            40,
            "Should aggregate stone production from both buildings"
        );

        // Building 1 used all 10 wood (0 unused).
        // Building 2 received 15 wood, used 10, returning 5. -> Total 5 unused wood.
        assert_eq!(
            *returned.get(&ResourceType::Wood).unwrap_or(&0),
            5,
            "Should aggregate the 5 leftover wood from building 2"
        );

        // Labour should not be returned as unused material by standard ProductionResult
        assert!(
            !returned.contains_key(&ResourceType::Labour),
            "Labour is consumed by the building's current_workers state, not returned as a material"
        );
    }
}