use crate::domain::buildings::core::{Building, BuildingLocation};
use crate::domain::economy::resource::{ResourceInventory, ResourceType};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Settlement {
    pub id: u32,
    pub resource_inventory: ResourceInventory,
    pub pending_production: ResourceInventory, // Buffer
    pub hub_buildings: Vec<Building>,
    pub spoke_buildings: Vec<Building>,
}

impl Settlement {
    pub fn new(id: u32, initial_capacity: f32) -> Self {
        Self {
            id,
            resource_inventory: ResourceInventory::new(initial_capacity),
            pending_production: ResourceInventory::new(initial_capacity),
            hub_buildings: Vec::new(),
            spoke_buildings: Vec::new(),
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

    pub fn process_turn(&mut self) {
        // --- 1: Collecting demands ---
        let mut total_demand: HashMap<ResourceType, f32> = HashMap::new();

        let all_buildings = self.hub_buildings.iter().chain(self.spoke_buildings.iter());
        for building in all_buildings {
            for (res, amount) in building.calculate_demand() {
                *total_demand.entry(res).or_insert(0.0) += amount;
            }
        }

        // --- 2: Calculating assigment ---
        let mut allocation_ratios: HashMap<ResourceType, f32> = HashMap::new();

        for (res, demand_amount) in total_demand {
            if demand_amount > 0.0 {
                let available = self
                    .resource_inventory
                    .resources
                    .get(&res)
                    .copied()
                    .unwrap_or(0.0);

                let ratio = (available / demand_amount).min(1.0);
                allocation_ratios.insert(res, ratio);
            }
        }

        // --- 3: Exec and buffering ---
        let mut all_produced: HashMap<ResourceType, f32> = HashMap::new();

        for building in self.hub_buildings.iter().chain(self.spoke_buildings.iter()) {
            let produced =
                building.execute_production(&mut self.resource_inventory, &allocation_ratios);
            for (res, amount) in produced {
                *all_produced.entry(res).or_insert(0.0) += amount;
            }
        }

        for (res, amount) in all_produced {
            self.pending_production.add(res, amount);
        }

        // --- 4: Archiving at the turn end ---
        for (res, amount) in self.pending_production.resources.drain() {
            self.resource_inventory.add(res, amount);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::buildings::core::Position;
    use crate::domain::buildings::factory::{BuildingBehavior, BuildingDefinition};
    use crate::domain::economy::resource::ResourceType;
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

    #[test]
    fn test_settlement_initialization() {
        let settlement = Settlement::new(42, 500.0);

        assert_eq!(settlement.id, 42);
        assert_eq!(settlement.resource_inventory.max_capacity, 500.0);
        assert!(settlement.hub_buildings.is_empty());
        assert!(settlement.spoke_buildings.is_empty());
    }

    #[test]
    fn test_construct_building_routing() {
        let mut settlement = Settlement::new(1, 100.0);

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
        let mut settlement = Settlement::new(1, 1000.0);

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
        settlement.process_turn();

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
        settlement.process_turn();

        {
            let inv = &settlement.resource_inventory.resources;
            assert_eq!(
                *inv.get(&ResourceType::Wood).unwrap_or(&0.0),
                10.0,
                "Hub consumed old wood."
            );
            assert_eq!(
                *inv.get(&ResourceType::Stone).unwrap_or(&0.0),
                5.0,
                "Hub used wood produced in T1"
            );
        }
    }
}
