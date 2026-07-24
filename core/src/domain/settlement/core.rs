use crate::domain::buildings::core::{Building, BuildingLocation};
use crate::domain::economy::resource::ResourceInventory;
use crate::domain::economy::resource_allocation::{AllocationEngine, DemandRequest, Priority};
use crate::domain::settlement::demographics::{ConsumptionResult, NeedRegistry, PopulationManager};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct Settlement {
    pub id: u32,
    pub resource_inventory: ResourceInventory,
    pub pending_production: ResourceInventory, // Buffer
    pub hub_buildings: Vec<Building>,
    pub spoke_buildings: Vec<Building>,
    pub population_manager: PopulationManager,
}

impl Settlement {
    pub fn new(
        id: u32,
        initial_capacity: f32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
    ) -> Self {
        Self {
            id,
            resource_inventory: ResourceInventory::new(initial_capacity),
            pending_production: ResourceInventory::new(initial_capacity),
            hub_buildings: Vec::new(),
            spoke_buildings: Vec::new(),
            population_manager: PopulationManager::new(initial_population, need_registry),
        }
    }

    pub fn construct_building(&mut self, building: Building) -> Result<(), &'static str> {
        let loc = building.location;

        match loc {
            BuildingLocation::Hub => self.hub_buildings.push(building),
            BuildingLocation::Spoke => self.spoke_buildings.push(building),
        }

        Ok(())
    }

    pub fn process_turn(&mut self) -> ConsumptionResult {
        let mut all_requests: Vec<DemandRequest> = Vec::new();

        all_requests.extend(self.population_manager.generate_requests());

        for building in self.hub_buildings.iter().chain(self.spoke_buildings.iter()) {
            let demand = building.calculate_demand();
            if !demand.is_empty() {
                all_requests.push(DemandRequest {
                    entity_id: building.instance_id,
                    priority: Priority::Industry,
                    demand,
                });
            }
        }

        let allocations = AllocationEngine::execute(&mut self.resource_inventory, all_requests);

        let pop_grants = allocations.get(&0);
        let consumption_result = self.population_manager.process_allocation(pop_grants);

        let mut all_produced = HashMap::new();
        let mut all_returned = HashMap::new();

        for building in self.hub_buildings.iter().chain(self.spoke_buildings.iter()) {
            let grants = allocations.get(&building.instance_id);
            let prod_result = building.execute_production(grants);

            for (res, amount) in prod_result.produced {
                *all_produced.entry(res).or_insert(0.0) += amount;
            }

            for (res, amount) in prod_result.unused {
                if amount > 0.0 {
                    *all_returned.entry(res).or_insert(0.0) += amount;
                }
            }
        }

        for (res, amount) in all_returned {
            self.resource_inventory.add(res, amount);
        }

        for (res, amount) in all_produced {
            self.pending_production.add(res, amount);
        }

        for (res, amount) in self.pending_production.resources.drain() {
            self.resource_inventory.add(res, amount);
        }

        consumption_result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::buildings::core::{BuildingLocation, Position};
    use crate::domain::buildings::factory::{BuildingBehavior, BuildingDefinition};
    use crate::domain::economy::resource::ResourceType;
    use crate::domain::settlement::demographics::NeedRegistry;
    use std::sync::Arc;

    /// Helper for building creation
    fn create_dummy_building(
        instance_id: u32,
        location: BuildingLocation,
        behavior: Option<BuildingBehavior>,
    ) -> Building {
        let behaviors = if let Some(b) = behavior {
            vec![b]
        } else {
            vec![]
        };

        let def = Arc::new(BuildingDefinition {
            id: "test_building".to_string(),
            name: "Test Building".to_string(),
            tier: 1,
            max_workers: 10,
            behaviors,
        });

        let mut building = Building::new(instance_id, Position { x: 0, y: 0 }, location, def);
        building.current_workers = 10;
        building
    }

    /// Helper to provide a dummy registry for settlement initialization
    fn create_test_registry() -> Arc<NeedRegistry> {
        Arc::new(NeedRegistry::new(vec![])) // Empty registry for building-focused tests
    }

    #[test]
    fn test_settlement_initialization() {
        let registry = create_test_registry();
        // Updated to include initial population (0) and the registry
        let settlement = Settlement::new(42, 500.0, 0, registry);

        assert_eq!(settlement.id, 42);
        assert_eq!(settlement.resource_inventory.max_capacity, 500.0);
        assert!(settlement.hub_buildings.is_empty());
        assert!(settlement.spoke_buildings.is_empty());
        assert_eq!(settlement.population_manager.population, 0);
    }

    #[test]
    fn test_construct_building_routing() {
        let registry = create_test_registry();
        let mut settlement = Settlement::new(1, 100.0, 0, registry);

        let hub_building = create_dummy_building(100, BuildingLocation::Hub, None);
        let spoke_building = create_dummy_building(101, BuildingLocation::Spoke, None);

        let res_hub = settlement.construct_building(hub_building);
        let res_spoke = settlement.construct_building(spoke_building);

        assert!(res_hub.is_ok());
        assert!(res_spoke.is_ok());

        assert_eq!(settlement.hub_buildings.len(), 1);
        assert_eq!(settlement.hub_buildings[0].instance_id, 100);

        assert_eq!(settlement.spoke_buildings.len(), 1);
        assert_eq!(settlement.spoke_buildings[0].instance_id, 101);
    }

    #[test]
    fn test_process_turn_delayed_production_no_race_conditions() {
        let registry = create_test_registry();
        let mut settlement = Settlement::new(1, 1000.0, 0, registry);

        let spoke_behavior = BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(ResourceType::Wood, 10.0)],
        };
        let spoke_building =
            create_dummy_building(10, BuildingLocation::Spoke, Some(spoke_behavior));

        let hub_behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 10.0)],
            outputs: vec![(ResourceType::Stone, 5.0)],
        };
        let hub_building = create_dummy_building(11, BuildingLocation::Hub, Some(hub_behavior));

        settlement.construct_building(spoke_building).unwrap();
        settlement.construct_building(hub_building).unwrap();

        assert_eq!(settlement.resource_inventory.total_amount(), 0.0);

        // --- TURN 1 ---
        let _ = settlement.process_turn(); // Ignoring ConsumptionResult as population is 0

        {
            let inv = &settlement.resource_inventory.resources;
            assert_eq!(
                *inv.get(&ResourceType::Wood).unwrap_or(&0.0),
                10.0,
                "Produced wood is available at the turn end."
            );
            assert_eq!(
                *inv.get(&ResourceType::Stone).unwrap_or(&0.0),
                0.0,
                "Hub didn't have wood."
            );
        }

        // --- TURN 2 ---
        let _ = settlement.process_turn();

        {
            let inv = &settlement.resource_inventory.resources;
            assert_eq!(
                *inv.get(&ResourceType::Wood).unwrap_or(&0.0),
                10.0,
                "Hub consumed old wood, but Spoke produced a new batch of 10."
            );
            assert_eq!(
                *inv.get(&ResourceType::Stone).unwrap_or(&0.0),
                5.0,
                "Hub used wood produced in T1"
            );
        }
    }
}
