use crate::domain::buildings::factory::{BuildingBehavior, BuildingDefinition};
use crate::domain::economy::resource::{ResourceInventory, ResourceType};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingLocation {
    Hub,
    Spoke,
}

#[derive(Debug)]
pub struct Building {
    pub instance_id: u32,
    pub position: Position,
    pub location: BuildingLocation,
    pub definition: Arc<BuildingDefinition>,
    pub current_workers: u32,
    pub is_active: bool,
}

impl Building {
    pub fn new(
        instance_id: u32,
        position: Position,
        location: BuildingLocation,
        definition: Arc<BuildingDefinition>,
    ) -> Self {
        Self {
            instance_id,
            position,
            location,
            definition,
            current_workers: 0,
            is_active: true,
        }
    }

    pub fn upgrade(&mut self, new_definition: Arc<BuildingDefinition>) {
        self.definition = new_definition;
    }

    pub fn calculate_demand(&self) -> HashMap<ResourceType, f32> {
        let mut demand = HashMap::new();

        if !self.is_active || self.current_workers == 0 {
            return demand;
        }

        let efficiency = self.current_workers as f32 / self.definition.max_workers as f32;

        for behavior in &self.definition.behaviors {
            if let BuildingBehavior::Production { inputs, .. } = behavior {
                for (res, amount) in inputs {
                    *demand.entry(*res).or_insert(0.0) += amount * efficiency;
                }
            }
        }
        demand
    }

    pub fn execute_production(
        &self,
        inventory: &mut ResourceInventory,
        allocation_ratios: &HashMap<ResourceType, f32>,
    ) -> HashMap<ResourceType, f32> {
        let mut produced = HashMap::new();

        if !self.is_active || self.current_workers == 0 {
            return produced;
        }

        let efficiency = self.current_workers as f32 / self.definition.max_workers as f32;

        for behavior in &self.definition.behaviors {
            if let BuildingBehavior::Production { inputs, outputs } = behavior {
                let mut bottleneck_ratio = 1.0;
                for (res, _) in inputs {
                    let ratio = allocation_ratios.get(res).copied().unwrap_or(0.0);
                    if ratio < bottleneck_ratio {
                        bottleneck_ratio = ratio;
                    }
                }

                if bottleneck_ratio <= 0.0 {
                    continue;
                }

                for (res, amount) in inputs {
                    let to_consume = amount * efficiency * bottleneck_ratio;
                    inventory.consume(*res, to_consume);
                }

                for (res, amount) in outputs {
                    let to_produce = amount * efficiency * bottleneck_ratio;
                    *produced.entry(*res).or_insert(0.0) += to_produce;
                }
            }
        }
        produced
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::resource::ResourceType;
    use std::collections::HashMap;

    fn create_test_def(
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
        })
    }

    #[test]
    fn test_building_initialization() {
        let def = create_test_def("house", 5, vec![]);
        let building = Building::new(
            1,
            Position { x: 10, y: 15 },
            BuildingLocation::Hub,
            def.clone(),
        );

        assert_eq!(building.instance_id, 1);
        assert_eq!(building.position, Position { x: 10, y: 15 });
        assert_eq!(building.location, BuildingLocation::Hub);
        assert_eq!(
            building.current_workers, 0,
            "New building should not have assigned workers."
        );
        assert!(building.is_active, "New building should be active.");
        assert!(Arc::ptr_eq(&building.definition, &def));
    }

    #[test]
    fn test_building_upgrade() {
        let def_t1 = create_test_def("lumber_t1", 5, vec![]);
        let def_t2 = create_test_def("lumber_t2", 10, vec![]);

        let mut building = Building::new(
            1,
            Position { x: 0, y: 0 },
            BuildingLocation::Spoke,
            def_t1.clone(),
        );

        building.upgrade(def_t2.clone());

        assert!(
            Arc::ptr_eq(&building.definition, &def_t2),
            "Ref should point at tier 2 building."
        );
        assert!(!Arc::ptr_eq(&building.definition, &def_t1));
    }

    #[test]
    fn test_execute_production_no_workers_no_production() {
        let behavior = BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(ResourceType::Wood, 10.0)],
        };
        let def = create_test_def("free_wood", 5, vec![behavior]);
        let building = Building::new(1, Position { x: 0, y: 0 }, BuildingLocation::Hub, def);

        let mut inventory = ResourceInventory::new(100.0);
        let ratios = HashMap::new();

        let produced = building.execute_production(&mut inventory, &ratios);

        assert!(
            produced.is_empty(),
            "Building without workers should not produce anything."
        );
    }

    #[test]
    fn test_execute_production_inactive_building() {
        let behavior = BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(ResourceType::Wood, 10.0)],
        };
        let def = create_test_def("free_wood", 5, vec![behavior]);
        let mut building = Building::new(1, Position { x: 0, y: 0 }, BuildingLocation::Hub, def);
        building.current_workers = 5;
        building.is_active = false;

        let mut inventory = ResourceInventory::new(100.0);
        let ratios = HashMap::new();

        let produced = building.execute_production(&mut inventory, &ratios);

        assert!(
            produced.is_empty(),
            "Inactive building should not produce anything."
        );
    }

    #[test]
    fn test_execute_production_full_efficiency() {
        let behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 10.0)],
            outputs: vec![(ResourceType::Stone, 20.0)],
        };
        let def = create_test_def("stone_maker", 10, vec![behavior]);
        let mut building = Building::new(1, Position { x: 0, y: 0 }, BuildingLocation::Spoke, def);
        building.current_workers = 10; // 100% efficiency

        let mut inventory = ResourceInventory::new(100.0);
        inventory.add(ResourceType::Wood, 15.0);

        let mut ratios = HashMap::new();
        ratios.insert(ResourceType::Wood, 1.0);

        let produced = building.execute_production(&mut inventory, &ratios);

        assert_eq!(
            *inventory.resources.get(&ResourceType::Wood).unwrap_or(&0.0),
            5.0
        );
        assert_eq!(*produced.get(&ResourceType::Stone).unwrap_or(&0.0), 20.0);
    }

    #[test]
    fn test_execute_production_partial_efficiency() {
        let behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 20.0)],
            outputs: vec![(ResourceType::Stone, 40.0)],
        };
        let def = create_test_def("stone_maker", 10, vec![behavior]);
        let mut building = Building::new(1, Position { x: 0, y: 0 }, BuildingLocation::Spoke, def);
        building.current_workers = 5;

        let mut inventory = ResourceInventory::new(100.0);
        inventory.add(ResourceType::Wood, 20.0);

        let mut ratios = HashMap::new();
        ratios.insert(ResourceType::Wood, 1.0);

        let produced = building.execute_production(&mut inventory, &ratios);

        assert_eq!(
            *inventory.resources.get(&ResourceType::Wood).unwrap_or(&0.0),
            10.0
        );
        assert_eq!(*produced.get(&ResourceType::Stone).unwrap_or(&0.0), 20.0);
    }

    #[test]
    fn test_execute_production_fractional_production_with_bottleneck() {
        let behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 10.0), (ResourceType::Grain, 5.0)],
            outputs: vec![(ResourceType::Stone, 20.0)],
        };

        let def = Arc::new(BuildingDefinition {
            id: "demanding_maker".to_string(),
            name: "Test Building".to_string(),
            tier: 1,
            max_workers: 10,
            behaviors: vec![behavior],
        });

        let mut building = Building::new(1, Position { x: 0, y: 0 }, BuildingLocation::Spoke, def);
        building.current_workers = 10; // Efficiency = 100%

        let mut inventory = ResourceInventory::new(100.0);
        inventory.add(ResourceType::Wood, 15.0);
        inventory.add(ResourceType::Grain, 2.0);

        let mut ratios = HashMap::new();
        ratios.insert(ResourceType::Wood, 1.0);
        ratios.insert(ResourceType::Grain, 0.4);

        let produced = building.execute_production(&mut inventory, &ratios);

        assert_eq!(
            *inventory
                .resources
                .get(&ResourceType::Grain)
                .unwrap_or(&0.0),
            0.0
        );
        assert_eq!(
            *inventory.resources.get(&ResourceType::Wood).unwrap_or(&0.0),
            11.0
        );
        assert_eq!(*produced.get(&ResourceType::Stone).unwrap_or(&0.0), 8.0);
    }
}
