use crate::domain::buildings::factory::{BuildingBehavior, BuildingDefinition, DefinitionError};
use crate::domain::buildings::productivity::LocationProductivity;
use crate::domain::economy::resource::ResourceType;
use crate::domain::ids::BuildingId;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

pub struct BuildingUpdateResult {
    pub production: ProductionResult,
    //pub modifiers: HashMap<ModifierType, f32>,
}

pub struct ProductionResult {
    pub produced: HashMap<ResourceType, u32>,
    pub unused: HashMap<ResourceType, u32>,
}

/// Identity and runtime state are protected by read accessors and validated commands.
/// ```compile_fail
/// use core::domain::buildings::core::Building;
/// fn change_identity(building: &mut Building) {
///     building.instance_id = core::domain::ids::BuildingId(10);
/// }
/// ```
#[derive(Debug)]
pub struct Building {
    instance_id: BuildingId,
    position: Position,
    definition: Arc<BuildingDefinition>,
    target_load: f32,
    location_productivity: LocationProductivity,
    is_active: bool,
    current_workers: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadError;
impl Building {
    pub fn instance_id(&self) -> BuildingId {
        self.instance_id
    }
    pub fn position(&self) -> Position {
        self.position
    }
    pub fn definition(&self) -> &BuildingDefinition {
        &self.definition
    }
    pub fn target_load(&self) -> f32 {
        self.target_load
    }
    /// Output multiplier, separate from staffing and recipe input demand.
    pub fn location_productivity(&self) -> f64 {
        self.location_productivity.as_factor()
    }

    pub(crate) fn set_location_productivity(&mut self, productivity: LocationProductivity) {
        self.location_productivity = productivity;
    }
    pub fn is_active(&self) -> bool {
        self.is_active
    }
    pub fn current_workers(&self) -> u32 {
        self.current_workers
    }
    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    /// Requested fraction of staffing and recipe inputs, in the range 0..=1.
    pub fn set_target_load(&mut self, value: f32) -> Result<(), LoadError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(LoadError);
        }
        self.target_load = value;
        Ok(())
    }

    pub fn new(
        instance_id: BuildingId,
        position: Position,
        definition: Arc<BuildingDefinition>,
    ) -> Result<Self, DefinitionError> {
        definition.validate()?;
        Ok(Self {
            instance_id,
            position,
            definition,
            target_load: 0.0,
            location_productivity: LocationProductivity::FULL,
            is_active: true,
            current_workers: 0,
        })
    }

    pub fn upgrade(
        &mut self,
        new_definition: Arc<BuildingDefinition>,
    ) -> Result<(), DefinitionError> {
        new_definition.validate()?;
        self.current_workers = self.current_workers.min(new_definition.max_workers);
        self.definition = new_definition;
        Ok(())
    }

    pub fn calculate_demand(&self) -> HashMap<ResourceType, u32> {
        let mut demand = HashMap::new();

        if !self.is_active {
            return demand;
        }

        let labor_needed =
            (self.definition.max_workers as f64 * f64::from(self.target_load)).floor() as u32;
        if labor_needed > 0 {
            demand.insert(ResourceType::Labour, labor_needed);
        }

        for behavior in &self.definition.behaviors {
            if let BuildingBehavior::Production { inputs, .. } = behavior {
                for (res, amount) in inputs {
                    *demand.entry(*res).or_insert(0) +=
                        (*amount as f64 * f64::from(self.target_load)).floor() as u32;
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
            self.current_workers = labor.min(self.definition.max_workers);
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

        // Recipes run in definition order against the remaining material balance.
        // Exact fractions avoid floating-point rounding at the withdrawal boundary.
        for behavior in &self.definition.behaviors {
            if let BuildingBehavior::Production { inputs, outputs } = behavior {
                let mut required: HashMap<ResourceType, u64> = HashMap::new();
                for &(resource, amount) in inputs {
                    *required.entry(resource).or_default() += u64::from(amount);
                }
                let mut numerator = u128::from(self.current_workers);
                let mut denominator = u128::from(self.definition.max_workers);
                for (&resource, &amount) in &required {
                    let available = u128::from(unused.get(&resource).copied().unwrap_or(0));
                    if amount > 0 && available * denominator < u128::from(amount) * numerator {
                        numerator = available;
                        denominator = u128::from(amount);
                    }
                }
                if numerator == 0 {
                    continue;
                }
                // Aggregate repeated inputs before rounding so each resource is
                // withdrawn once, never exceeding its remaining grant.
                for (resource, amount) in required {
                    let consumed = (u128::from(amount) * numerator).div_ceil(denominator);
                    if consumed > 0 {
                        let remaining = unused.get_mut(&resource).expect("input must be granted");
                        *remaining = remaining
                            .checked_sub(
                                u32::try_from(consumed).expect("consumption cannot exceed grant"),
                            )
                            .expect("recipe cannot consume more than its remaining grant");
                    }
                }
                for &(resource, amount) in outputs {
                    let output =
                        self.location_productivity
                            .scale_output(amount, numerator, denominator);
                    let total = produced.entry(resource).or_insert(0_u32);
                    *total = total
                        .checked_add(output)
                        .expect("production exceeds u32 capacity");
                }
            }
        }
        ProductionResult { produced, unused }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::resource::ResourceType;
    use std::collections::HashMap;

    #[test]
    fn repeated_inputs_are_aggregated_before_consumption() {
        let def = create_test_def(
            "repeated",
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![(ResourceType::Wood, 1), (ResourceType::Wood, 1)],
                outputs: vec![(ResourceType::Stone, 2)],
            }],
        );
        let mut building = Building::new(BuildingId(1), Position { x: 0, y: 0 }, def).unwrap();
        let grants = HashMap::from([(ResourceType::Labour, 1), (ResourceType::Wood, 1)]);
        let result = building.resolve_allocation(Some(&grants)).production;
        assert_eq!(result.produced[&ResourceType::Stone], 1);
        assert_eq!(result.unused[&ResourceType::Wood], 0);
    }

    #[test]
    fn fractional_input_costs_never_exceed_remaining_grants() {
        let def = create_test_def(
            "fractional",
            3,
            vec![BuildingBehavior::Production {
                inputs: vec![(ResourceType::Wood, 7), (ResourceType::Grain, 3)],
                outputs: vec![(ResourceType::Stone, 9)],
            }],
        );
        let mut building = Building::new(BuildingId(1), Position { x: 0, y: 0 }, def).unwrap();
        let grants = HashMap::from([
            (ResourceType::Labour, 1),
            (ResourceType::Wood, 3),
            (ResourceType::Grain, 1),
        ]);
        let result = building.resolve_allocation(Some(&grants)).production;
        assert_eq!(result.produced[&ResourceType::Stone], 3);
        assert_eq!(result.unused[&ResourceType::Wood], 0);
        assert_eq!(result.unused[&ResourceType::Grain], 0);
    }

    #[test]
    fn large_recipe_is_resolved_without_float_rounding() {
        let amount = 16_777_219;
        let def = create_test_def(
            "large",
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![(ResourceType::Wood, amount)],
                outputs: vec![(ResourceType::Stone, amount)],
            }],
        );
        let mut building = Building::new(BuildingId(1), Position { x: 0, y: 0 }, def).unwrap();
        let grants = HashMap::from([(ResourceType::Labour, 1), (ResourceType::Wood, amount)]);
        let result = building.resolve_allocation(Some(&grants)).production;
        assert_eq!(result.produced[&ResourceType::Stone], amount);
        assert_eq!(result.unused[&ResourceType::Wood], 0);
    }

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
        let building =
            Building::new(BuildingId(1), Position { x: 10, y: 15 }, def.clone()).unwrap();

        assert_eq!(building.instance_id, BuildingId(1));
        assert_eq!(building.position, Position { x: 10, y: 15 });

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

        let mut building =
            Building::new(BuildingId(1), Position { x: 0, y: 0 }, def_t1.clone()).unwrap();

        building.upgrade(def_t2.clone()).unwrap();

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
        let mut building = Building::new(BuildingId(1), Position { x: 0, y: 0 }, def).unwrap();

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
        let mut building = Building::new(BuildingId(1), Position { x: 0, y: 0 }, def).unwrap();
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
        let mut building = Building::new(BuildingId(1), Position { x: 0, y: 0 }, def).unwrap();

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
        let mut building = Building::new(BuildingId(1), Position { x: 0, y: 0 }, def).unwrap();

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

        let mut building = Building::new(BuildingId(1), Position { x: 0, y: 0 }, def).unwrap();

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
