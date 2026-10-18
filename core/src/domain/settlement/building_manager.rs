use crate::domain::buildings::core::Building;
use crate::domain::buildings::core::Position;
use crate::domain::buildings::factory::BuildingDefinition;
use crate::domain::buildings::productivity::{
    BuildingProductivityCalculator, LocationProductivity,
};
use crate::domain::economy::resource::ResourceType;
use crate::domain::economy::resource_allocation::{
    AllocationRecipient, AllocationResult, DemandRequest,
};
use crate::domain::ids::BuildingId;
use crate::domain::world_data::map::Map;
use crate::services::algorithms::path_finder::PathFinder;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct BuildingManager {
    pub(super) buildings: Vec<Building>,
    productivity_calculator: Option<BuildingProductivityCalculator>,
}

impl BuildingManager {
    pub fn new() -> Self {
        Self {
            buildings: Vec::new(),
            productivity_calculator: None,
        }
    }

    pub fn construct_building(
        &mut self,
        mut building: Building,
        map: &Map,
        finder: &mut PathFinder,
    ) -> Result<(), BuildingId> {
        if self.contains(building.instance_id()) {
            return Err(building.instance_id());
        }
        building.set_location_productivity(self.productivity_at(building.position(), map, finder));
        self.buildings.push(building);
        Ok(())
    }

    pub fn productivity_calculator(&self) -> Option<BuildingProductivityCalculator> {
        self.productivity_calculator
    }

    pub fn set_productivity_calculator(
        &mut self,
        calculator: Option<BuildingProductivityCalculator>,
        map: &Map,
        finder: &mut PathFinder,
    ) {
        self.productivity_calculator = calculator;
        self.refresh_productivity(map, finder);
    }

    pub fn refresh_productivity(&mut self, map: &Map, finder: &mut PathFinder) {
        let calculator = self.productivity_calculator;
        for building in &mut self.buildings {
            let productivity = calculator.map_or(LocationProductivity::FULL, |calculator| {
                calculator.multiplier_at(building.position(), map, finder)
            });
            building.set_location_productivity(productivity);
        }
    }

    fn productivity_at(
        &self,
        position: Position,
        map: &Map,
        finder: &mut PathFinder,
    ) -> LocationProductivity {
        self.productivity_calculator
            .map_or(LocationProductivity::FULL, |calculator| {
                calculator.multiplier_at(position, map, finder)
            })
    }

    pub fn contains(&self, id: BuildingId) -> bool {
        self.buildings
            .iter()
            .any(|building| building.instance_id() == id)
    }

    /// A completed project receives a building ID independent of its project ID.
    pub fn next_available_id(&self) -> BuildingId {
        let used: std::collections::HashSet<_> = self
            .buildings
            .iter()
            .map(|building| building.instance_id())
            .collect();
        (0..=u32::MAX)
            .map(BuildingId)
            .find(|id| !used.contains(id))
            .expect("building ID space exhausted")
    }

    pub fn upgrade_building(
        &mut self,
        target_id: BuildingId,
        new_def: Arc<BuildingDefinition>,
    ) -> bool {
        if let Some(building) = self
            .buildings
            .iter_mut()
            .find(|b| b.instance_id() == target_id)
        {
            building.upgrade(new_def).is_ok()
        } else {
            false
        }
    }

    pub fn generate_requests(&self) -> Vec<DemandRequest> {
        self.buildings
            .iter()
            .filter_map(|b| {
                let demand = b.calculate_demand();
                if demand.is_empty() {
                    None
                } else {
                    Some(DemandRequest {
                        recipient: AllocationRecipient::Building(b.instance_id()),
                        demand,
                    })
                }
            })
            .collect()
    }

    pub fn process_allocations(
        &mut self,
        allocations: &AllocationResult,
    ) -> (HashMap<ResourceType, u64>, HashMap<ResourceType, u64>) {
        let mut all_produced = HashMap::new();
        let mut all_returned = HashMap::new();

        for building in self.buildings.iter_mut() {
            let grants = allocations.get(&AllocationRecipient::Building(building.instance_id()));
            let update_result = building.resolve_allocation(grants);

            for (res, amount) in update_result.production.produced {
                *all_produced.entry(res).or_insert(0_u64) += u64::from(amount);
            }
            for (res, amount) in update_result.production.unused {
                if amount > 0 {
                    *all_returned.entry(res).or_insert(0_u64) += u64::from(amount);
                }
            }
        }

        (all_produced, all_returned)
    }
}

#[cfg(test)]
mod tests {
    fn test_map() -> Map {
        Map::new(
            1,
            1,
            vec![crate::domain::geography::area::Area::new(0.5, 0.5)],
            vec![0],
        )
    }
    fn test_finder() -> PathFinder {
        PathFinder::new(1, 1)
    }

    use super::*;
    use crate::domain::buildings::core::{Building, Position};
    use crate::domain::buildings::factory::{BuildingBehavior, BuildingDefinition};
    use crate::domain::economy::resource::ResourceType;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn duplicate_building_ids_are_rejected() {
        let mut manager = BuildingManager::new();
        let def = create_test_definition("test", 1, vec![]);
        manager
            .construct_building(
                create_active_building(7, def.clone()),
                &test_map(),
                &mut test_finder(),
            )
            .unwrap();
        let duplicate = create_active_building(7, def);
        assert_eq!(
            manager.construct_building(duplicate, &test_map(), &mut test_finder()),
            Err(BuildingId(7))
        );
        assert_eq!(manager.buildings.len(), 1);
    }

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

    fn create_active_building(instance_id: u32, def: Arc<BuildingDefinition>) -> Building {
        let mut building =
            Building::new(BuildingId(instance_id), Position { x: 0, y: 0 }, def).unwrap();
        // Important: default is 0.0, which means no demand and no production.
        // We set it to 1.0 to simulate a fully functioning building.
        building.set_target_load(1.0).unwrap();
        building
    }

    // --- Tests ---

    #[test]
    fn construction_preserves_building_insertion_order() {
        // Arrange
        let mut manager = BuildingManager::new();
        let def = create_test_definition("house", 0, vec![]);

        let first = create_active_building(1, Arc::clone(&def));
        let second = create_active_building(2, Arc::clone(&def));

        // Act
        manager
            .construct_building(first, &test_map(), &mut test_finder())
            .unwrap();
        manager
            .construct_building(second, &test_map(), &mut test_finder())
            .unwrap();

        // Assert
        assert_eq!(manager.buildings.len(), 2);
        assert_eq!(manager.buildings[0].instance_id(), BuildingId(1));

        assert_eq!(manager.buildings[1].instance_id(), BuildingId(2));
    }

    #[test]
    fn test_upgrade_building_success() {
        // Arrange
        let mut manager = BuildingManager::new();
        let old_def = create_test_definition("tier_1", 5, vec![]);
        let new_def = create_test_definition("tier_2", 10, vec![]);

        manager
            .construct_building(
                create_active_building(1, old_def),
                &test_map(),
                &mut test_finder(),
            )
            .unwrap();

        // Act
        let result = manager.upgrade_building(BuildingId(1), Arc::clone(&new_def));

        // Assert
        assert!(result, "Upgrade should return true for existing building");
        assert!(
            std::ptr::eq(manager.buildings[0].definition(), new_def.as_ref()),
            "Building definition should be successfully replaced"
        );
    }

    #[test]
    fn test_upgrade_building_not_found() {
        // Arrange
        let mut manager = BuildingManager::new();
        let def = create_test_definition("tier_1", 5, vec![]);
        manager
            .construct_building(
                create_active_building(1, def),
                &test_map(),
                &mut test_finder(),
            )
            .unwrap();

        let new_def = create_test_definition("tier_2", 10, vec![]);

        // Act
        let result = manager.upgrade_building(BuildingId(99), new_def);

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

        // Building 1 wants 5 Labour.
        let def1 = create_test_definition("house", 5, vec![]);
        manager
            .construct_building(
                create_active_building(1, def1),
                &test_map(),
                &mut test_finder(),
            )
            .unwrap();

        // Building 2 wants 10 Labour + 10 Wood (via Production behavior).
        let prod_behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 10)],
            outputs: vec![(ResourceType::Stone, 20)],
        };
        let def2 = create_test_definition("lumber", 10, vec![prod_behavior]);
        manager
            .construct_building(
                create_active_building(2, def2),
                &test_map(),
                &mut test_finder(),
            )
            .unwrap();

        // Act
        let requests = manager.generate_requests();

        // Assert
        assert_eq!(
            requests.len(),
            2,
            "Should generate exactly 2 demand requests"
        );

        // Verify Building 1 demand
        let req1 = requests
            .iter()
            .find(|r| r.recipient == AllocationRecipient::Building(BuildingId(1)))
            .unwrap();
        assert_eq!(*req1.demand.get(&ResourceType::Labour).unwrap(), 5);
        assert!(!req1.demand.contains_key(&ResourceType::Wood));

        // Verify Building 2 demand
        let req2 = requests
            .iter()
            .find(|r| r.recipient == AllocationRecipient::Building(BuildingId(2)))
            .unwrap();
        assert_eq!(*req2.demand.get(&ResourceType::Labour).unwrap(), 10);
        assert_eq!(*req2.demand.get(&ResourceType::Wood).unwrap(), 10);
    }

    #[test]
    fn test_process_allocations_aggregates_production_and_unused() {
        // Arrange
        let mut manager = BuildingManager::new();

        let behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 10)],
            outputs: vec![(ResourceType::Stone, 20)],
        };

        let def = create_test_definition("stone_maker", 10, vec![behavior]);

        // Add two identical buildings
        manager
            .construct_building(
                create_active_building(1, Arc::clone(&def)),
                &test_map(),
                &mut test_finder(),
            )
            .unwrap();
        manager
            .construct_building(
                create_active_building(2, def),
                &test_map(),
                &mut test_finder(),
            )
            .unwrap();

        // Setup AllocationResult
        let mut allocations: AllocationResult = HashMap::new();

        // Building 1 gets exactly what it needs
        let mut b1_grants = HashMap::new();
        b1_grants.insert(ResourceType::Labour, 10);
        b1_grants.insert(ResourceType::Wood, 10);
        allocations.insert(AllocationRecipient::Building(BuildingId(1)), b1_grants);

        // Building 2 gets full labour, but EXTRA wood (15 instead of 10)
        let mut b2_grants = HashMap::new();
        b2_grants.insert(ResourceType::Labour, 10);
        b2_grants.insert(ResourceType::Wood, 15);
        allocations.insert(AllocationRecipient::Building(BuildingId(2)), b2_grants);

        // Act
        let (produced, returned) = manager.process_allocations(&allocations);

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
