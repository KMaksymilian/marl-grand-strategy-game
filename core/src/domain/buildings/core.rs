use crate::domain::buildings::factory::{BuildingBehavior, BuildingDefinition};
use crate::domain::economy::resource::ResourceType;
use std::collections::HashMap;
use std::sync::Arc;
use crate::domain::world_data::point::Point;


pub struct BuildingUpdateResult {
    pub production: ProductionResult,
    //pub modifiers: HashMap<ModifierType, f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingLocation {
    Hub,
    Spoke,
}

pub struct ProductionResult {
    pub produced: HashMap<ResourceType, u32>,
    pub unused: HashMap<ResourceType, u32>,
}

#[derive(Debug)]
pub struct Building {
    pub instance_id: u32,
    pub position: Point,
    pub location: BuildingLocation,
    pub definition: Arc<BuildingDefinition>,
    pub target_efficiency: f32,
    pub is_active: bool,
    pub current_workers: u32,
}

impl Building {
    pub fn new(
        instance_id: u32,
        position: Point,
        location: BuildingLocation,
        definition: Arc<BuildingDefinition>,
    ) -> Self {
        Self {
            instance_id,
            position,
            location,
            definition,
            target_efficiency: 0.0,
            is_active: true,
            current_workers: 0,
        }
    }

    pub fn upgrade(&mut self, new_definition: Arc<BuildingDefinition>) {
        self.definition = new_definition;
    }

    pub fn calculate_demand(&self) -> HashMap<ResourceType, u32> {
        let mut demand = HashMap::new();

        if !self.is_active {
            return demand;
        }

        let labor_needed =
            (self.definition.max_workers as f32 * self.target_efficiency).floor() as u32;
        if labor_needed > 0 {
            demand.insert(ResourceType::Labour, labor_needed);
        }

        for behavior in &self.definition.behaviors {
            if let BuildingBehavior::Production { inputs, .. } = behavior {
                for (res, amount) in inputs {
                    *demand.entry(*res).or_insert(0) +=
                        (*amount as f32 * self.target_efficiency).floor() as u32;
                }
            }
        }
        demand
    }

    pub fn resolve_allocation(
        &mut self,
        granted: Option<&HashMap<ResourceType, u32>>,
    ) -> BuildingUpdateResult {
        let mut material_grants = granted.cloned().unwrap_or_default();

        if self.is_active && self.definition.max_workers > 0 {
            let labor = material_grants.remove(&ResourceType::Labour).unwrap_or(0);
            self.current_workers = labor;
        } else {
            self.current_workers = 0;
            material_grants.remove(&ResourceType::Labour);
        }

        let production = self.execute_production(Some(&material_grants));
        // let modifiers = self.execute_stat_modification();

        BuildingUpdateResult {
            production,
            //  modifiers,
        }
    }

    fn execute_production(&self, granted: Option<&HashMap<ResourceType, u32>>) -> ProductionResult {
        let empty_grants = HashMap::new();
        let granted_ref = granted.unwrap_or(&empty_grants);

        let mut produced = HashMap::new();
        let mut unused = granted_ref.clone();

        if !self.is_active || self.current_workers == 0 {
            return ProductionResult { produced, unused };
        }

        let efficiency = self.current_workers as f32 / self.definition.max_workers as f32;

        for behavior in &self.definition.behaviors {
            if let BuildingBehavior::Production { inputs, outputs } = behavior {
                // 1. Calculate bottleneck
                let bottleneck_ratio = Self::calculate_bottleneck(inputs, granted_ref, efficiency);

                if bottleneck_ratio <= 0.0 {
                    continue;
                }

                // 2. Consume input
                Self::consume_inputs(inputs, &mut unused, efficiency, bottleneck_ratio);

                // 3. Gen output
                Self::generate_outputs(outputs, &mut produced, efficiency, bottleneck_ratio);
            }
        }

        ProductionResult { produced, unused }
    }

    fn calculate_bottleneck(
        inputs: &[(ResourceType, u32)],
        granted: &HashMap<ResourceType, u32>,
        efficiency: f32,
    ) -> f32 {
        let mut bottleneck = 1.0;
        for (res, base_amount) in inputs {
            let required = *base_amount as f32 * efficiency;
            if required > 0.0 {
                let available = granted.get(res).copied().unwrap_or(0);
                let ratio = (available as f32 / required).min(1.0);
                if ratio < bottleneck {
                    bottleneck = ratio;
                }
            }
        }
        bottleneck
    }

    fn consume_inputs(
        inputs: &[(ResourceType, u32)],
        unused: &mut HashMap<ResourceType, u32>,
        efficiency: f32,
        bottleneck_ratio: f32,
    ) {
        for (res, base_amount) in inputs {
            let to_consume = (*base_amount as f32 * efficiency * bottleneck_ratio).ceil() as u32;
            if let Some(leftover) = unused.get_mut(res) {
                *leftover -= to_consume;
            }
        }
    }

    fn generate_outputs(
        outputs: &[(ResourceType, u32)],
        produced: &mut HashMap<ResourceType, u32>,
        efficiency: f32,
        bottleneck_ratio: f32,
    ) {
        for (res, base_amount) in outputs {
            let to_produce = (*base_amount as f32 * efficiency * bottleneck_ratio).floor() as u32;
            *produced.entry(*res).or_insert(0) += to_produce;
        }
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
            construction_cost: Default::default(),
        })
    }

    #[test]
    fn test_building_initialization() {
        let def = create_test_def("house", 5, vec![]);
        let building = Building::new(
            1,
            Point(10,15),
            BuildingLocation::Hub,
            def.clone(),
        );

        assert_eq!(building.instance_id, 1);
        assert_eq!(building.position, Point(10,15));
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
            Point(0,0),
            BuildingLocation::Spoke,
            def_t1.clone(),
        );

        building.upgrade(def_t2.clone());

        assert!(
            Arc::ptr_eq(&building.definition, &def_t2),
            "Ref should point at the tier 2 building."
        );
        assert!(!Arc::ptr_eq(&building.definition, &def_t1));
    }

    #[test]
    fn test_resolve_allocation_no_workers_no_production() {
        let behavior = BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(ResourceType::Wood, 10)],
        };
        let def = create_test_def("free_wood", 5, vec![behavior]);
        let mut building = Building::new(1, Point(0,0), BuildingLocation::Hub, def);

        // We do not grant Labour, only raw materials
        let mut granted = HashMap::new();
        granted.insert(ResourceType::Wood, 50);

        let result = building.resolve_allocation(Some(&granted));

        assert!(
            result.production.produced.is_empty(),
            "Building without workers should not produce anything."
        );
        assert_eq!(
            *result
                .production
                .unused
                .get(&ResourceType::Wood)
                .unwrap_or(&0),
            50,
            "All granted resources should be returned as unused."
        );
        assert_eq!(building.current_workers, 0);
    }

    #[test]
    fn test_resolve_allocation_inactive_building() {
        let behavior = BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(ResourceType::Wood, 10)],
        };
        let def = create_test_def("free_wood", 5, vec![behavior]);
        let mut building = Building::new(1, Point(0,0), BuildingLocation::Hub, def);
        building.is_active = false;

        let mut granted = HashMap::new();
        granted.insert(ResourceType::Labour, 5); // Attempt to assign workers
        granted.insert(ResourceType::Wood, 20);

        let result = building.resolve_allocation(Some(&granted));

        assert!(
            result.production.produced.is_empty(),
            "Inactive building should not produce anything."
        );
        assert_eq!(
            *result
                .production
                .unused
                .get(&ResourceType::Wood)
                .unwrap_or(&0),
            20,
            "Inactive building should return all granted material resources."
        );
        assert!(
            !result.production.unused.contains_key(&ResourceType::Labour),
            "Labour should not be returned as unused material, even if inactive."
        );
        assert_eq!(
            building.current_workers, 0,
            "Inactive building should reject workers."
        );
    }

    #[test]
    fn test_resolve_allocation_full_efficiency() {
        let behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 10)],
            outputs: vec![(ResourceType::Stone, 20)],
        };
        let def = create_test_def("stone_maker", 10, vec![behavior]);
        let mut building = Building::new(1, Point(0,0), BuildingLocation::Spoke, def);

        // AllocationEngine provides full labor and resources
        let mut granted = HashMap::new();
        granted.insert(ResourceType::Labour, 10);
        granted.insert(ResourceType::Wood, 10);

        let result = building.resolve_allocation(Some(&granted));

        assert_eq!(building.current_workers, 10);
        assert_eq!(
            *result
                .production
                .produced
                .get(&ResourceType::Stone)
                .unwrap_or(&0),
            20
        );
        assert_eq!(
            *result
                .production
                .unused
                .get(&ResourceType::Wood)
                .unwrap_or(&0),
            0,
            "All wood should be consumed."
        );
    }

    #[test]
    fn test_resolve_allocation_partial_efficiency() {
        let behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 20)],
            outputs: vec![(ResourceType::Stone, 40)],
        };
        let def = create_test_def("stone_maker", 10, vec![behavior]);
        let mut building = Building::new(1, Point(0,0), BuildingLocation::Spoke, def);

        // Grant only 50% of required workforce (5/10) and more wood than needed for 50% efficiency
        let mut granted = HashMap::new();
        granted.insert(ResourceType::Labour, 5);
        granted.insert(ResourceType::Wood, 15);

        let result = building.resolve_allocation(Some(&granted));

        assert_eq!(building.current_workers, 5);
        assert_eq!(
            *result
                .production
                .produced
                .get(&ResourceType::Stone)
                .unwrap_or(&0),
            20,
            "Should only produce 50% of max capacity."
        );
        assert_eq!(
            *result
                .production
                .unused
                .get(&ResourceType::Wood)
                .unwrap_or(&0),
            5,
            "Should return 5 leftover wood out of the 15 granted."
        );
    }

    #[test]
    fn test_resolve_allocation_fractional_production_with_bottleneck() {
        let behavior = BuildingBehavior::Production {
            inputs: vec![(ResourceType::Wood, 10), (ResourceType::Grain, 5)],
            outputs: vec![(ResourceType::Stone, 20)],
        };

        let def = Arc::new(BuildingDefinition {
            id: "demanding_maker".to_string(),
            name: "Test Building".to_string(),
            tier: 1,
            max_workers: 10,
            behaviors: vec![behavior],
            construction_cost: Default::default(),
        });

        let mut building = Building::new(1, Point(0,0), BuildingLocation::Spoke, def);

        // Workers = 100%
        // Wood = 100% (10 granted / 10 needed)
        // Grain = 40% (2 granted / 5 needed) -> Bottleneck is 0.4
        let mut granted = HashMap::new();
        granted.insert(ResourceType::Labour, 10);
        granted.insert(ResourceType::Wood, 10);
        granted.insert(ResourceType::Grain, 2);

        let result = building.resolve_allocation(Some(&granted));

        assert_eq!(building.current_workers, 10);

        // 1. Produced Stone should be 40% of 20 = 8.0
        assert_eq!(
            *result
                .production
                .produced
                .get(&ResourceType::Stone)
                .unwrap_or(&0),
            8
        );

        // 2. Consumed Grain is 2.0 (100% of granted). Unused = 0.0.
        assert_eq!(
            *result
                .production
                .unused
                .get(&ResourceType::Grain)
                .unwrap_or(&0),
            0
        );

        // 3. Consumed Wood is 4.0 (40% of needed). Unused = 10.0 - 4.0 = 6.0.
        assert_eq!(
            *result
                .production
                .unused
                .get(&ResourceType::Wood)
                .unwrap_or(&0),
            6
        );
    }
}
