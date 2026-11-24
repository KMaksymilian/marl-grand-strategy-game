mod config;

use crate::domain::economy::allocation::{AllocationRecipient, DemandRequest};
use crate::domain::economy::{ResourceInventory, ResourceType};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub use config::NeedsConfigError;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConsumptionResult {
    pub satisfaction_percent: f32,
    pub missing_resources: Vec<ResourceType>,
    /// Total units in the desired diet, independent of which foods were available.
    pub food_required: u64,
    pub food_eaten: u64,
    /// Equivalent number of people without a full ration, rounded up.
    pub people_unfed: u32,
}

#[derive(Debug, Clone)]
pub struct NeedTier {
    pub min_population: u32,
    pub needs_per_capita: HashMap<ResourceType, u32>,
}

#[derive(Debug, Default)]
pub struct NeedRegistry {
    tiers: Vec<NeedTier>,
    food: Option<FoodNeeds>,
}

#[derive(Debug)]
struct FoodNeeds {
    units_per_person: u32,
    items: Vec<ResourceType>,
    diets: BTreeMap<u32, Vec<DietShare>>,
}

#[derive(Debug)]
struct DietShare {
    resource: ResourceType,
    percent: u32,
}

impl NeedRegistry {
    pub fn new(mut tiers: Vec<NeedTier>) -> Self {
        tiers.sort_by_key(|tier| Reverse(tier.min_population));
        Self { tiers, food: None }
    }

    pub fn get_active_tiers(&self, population: u32) -> impl Iterator<Item = &NeedTier> {
        self.tiers
            .iter()
            .filter(move |tier| population >= tier.min_population)
    }

    pub(in crate::domain::settlement) fn materials(&self) -> impl Iterator<Item = &ResourceType> {
        self.tiers
            .iter()
            .flat_map(|tier| &tier.needs_per_capita)
            .filter(|(_, amount)| **amount > 0)
            .map(|(resource, _)| resource)
            .chain(self.food_items())
    }

    pub fn food_units_per_person(&self) -> Option<u32> {
        self.food.as_ref().map(|food| food.units_per_person)
    }

    pub fn food_items(&self) -> &[ResourceType] {
        self.food.as_ref().map_or(&[], |food| &food.items)
    }

    /// One turn of the desired diet, using the same whole-unit rounding as consumption.
    pub fn starting_food(&self, population: u32) -> HashMap<ResourceType, u32> {
        self.food_demand(population)
            .into_iter()
            .filter(|(_, amount)| *amount > 0)
            .map(|(resource, amount)| (resource, inventory_quantity(amount)))
            .collect()
    }

    fn food_demand(&self, population: u32) -> Vec<(ResourceType, u64)> {
        let Some(food) = &self.food else {
            return Vec::new();
        };
        let Some((_, diet)) = food.diets.range(..=population).next_back() else {
            return Vec::new();
        };
        let total = u64::from(food.units_per_person) * u64::from(population);
        let mut shares = Vec::with_capacity(diet.len());
        let mut remainders = Vec::with_capacity(diet.len());
        let mut rounded_total = 0;
        for (position, row) in diet.iter().enumerate() {
            let weighted = u128::from(total) * u128::from(row.percent);
            let amount = (weighted / 100) as u64;
            rounded_total += amount;
            shares.push((row.resource.clone(), amount));
            remainders.push((Reverse(weighted % 100), position));
        }
        remainders.sort_unstable();
        for (_, position) in remainders
            .into_iter()
            .take((total - rounded_total) as usize)
        {
            shares[position].1 += 1;
        }
        shares
    }
}

#[derive(Debug)]
pub struct PopulationManager {
    pub population: u32,
    pub registry: Arc<NeedRegistry>,
}

impl PopulationManager {
    pub fn new(population: u32, registry: Arc<NeedRegistry>) -> Self {
        Self {
            population,
            registry,
        }
    }

    pub fn generate_requests(&self) -> Vec<DemandRequest> {
        let demand = self.calculate_demand();
        if demand.is_empty() {
            return Vec::new();
        }
        vec![DemandRequest {
            recipient: AllocationRecipient::Population,
            demand,
        }]
    }

    /// Use the same whole resource units as inventories, recipes and projects.
    /// Request generation and satisfaction evaluation share this demand.
    pub(crate) fn calculate_demand(&self) -> HashMap<ResourceType, u32> {
        self.material_demand()
            .into_iter()
            .chain(self.registry.food_demand(self.population))
            .filter(|(_, amount)| *amount > 0)
            .map(|(resource, amount)| (resource, inventory_quantity(amount)))
            .collect()
    }

    fn material_demand(&self) -> HashMap<ResourceType, u64> {
        if self.population == 0 {
            return HashMap::new();
        }
        let mut per_capita = HashMap::new();
        for tier in self.registry.get_active_tiers(self.population) {
            for (resource, &amount) in &tier.needs_per_capita {
                let total = per_capita.entry(resource.clone()).or_insert(0_u32);
                *total = total
                    .checked_add(amount)
                    .expect("per-capita demand exceeds u32 capacity");
            }
        }
        per_capita
            .into_iter()
            .filter_map(|(resource, amount)| {
                let required = if self.registry.food.is_some() {
                    u64::from(amount) * u64::from(self.population)
                } else {
                    u64::from(
                        amount
                            .checked_mul(self.population)
                            .expect("population demand exceeds u32 capacity"),
                    )
                };
                (required > 0).then_some((resource, required))
            })
            .collect()
    }

    #[cfg(test)]
    pub fn process_allocation(
        &self,
        granted_resources: Option<&HashMap<ResourceType, u32>>,
    ) -> ConsumptionResult {
        self.consume_turn(granted_resources, &mut ResourceInventory::new(0))
            .0
    }

    /// Uses the allocation first, then covers any food shortfall from remaining stock.
    pub fn consume_turn(
        &self,
        grants: Option<&HashMap<ResourceType, u32>>,
        remaining_stock: &mut ResourceInventory,
    ) -> (ConsumptionResult, HashMap<ResourceType, u32>) {
        let mut material_required = 0_u128;
        let mut material_satisfied = 0_u128;
        let mut missing_resources = Vec::new();
        let received = |resource: &ResourceType| {
            u64::from(
                grants
                    .and_then(|grants| grants.get(resource))
                    .copied()
                    .unwrap_or(0),
            )
        };

        for (resource, required) in self.material_demand() {
            material_required += u128::from(required);
            let satisfied = received(&resource).min(required);
            material_satisfied += u128::from(satisfied);
            if satisfied < required {
                missing_resources.push(resource);
            }
        }

        let mut food_required = 0;
        let mut food_shortfall = 0;
        let mut missing_food = Vec::new();
        for (resource, required) in self.registry.food_demand(self.population) {
            food_required += required;
            let deficit = required.saturating_sub(received(&resource));
            if deficit > 0 {
                food_shortfall += deficit;
                missing_food.push((resource, deficit));
            }
        }
        let substituted =
            substitute_food(remaining_stock, self.registry.food_items(), food_shortfall);
        let mut substituted_units: u64 = substituted.values().copied().map(u64::from).sum();
        let uncovered = food_shortfall - substituted_units;
        for (resource, deficit) in missing_food {
            let covered = substituted_units.min(deficit);
            substituted_units -= covered;
            if covered < deficit {
                missing_resources.push(resource);
            }
        }
        missing_resources.sort_unstable();
        let food_eaten = food_required - uncovered;
        let people_unfed = self.registry.food_units_per_person().map_or(0, |units| {
            u32::try_from(uncovered.div_ceil(u64::from(units)))
                .expect("unfed people cannot outnumber the population")
        });
        let total_demanded = material_required + u128::from(food_required);
        let total_satisfied = material_satisfied + u128::from(food_eaten);
        let satisfaction_percent = if total_demanded > 0 {
            (total_satisfied as f64 / total_demanded as f64) as f32
        } else {
            1.0
        };

        (
            ConsumptionResult {
                satisfaction_percent,
                missing_resources,
                food_required,
                food_eaten,
                people_unfed,
            },
            substituted,
        )
    }
}

fn inventory_quantity(amount: u64) -> u32 {
    u32::try_from(amount).unwrap_or(u32::MAX)
}

fn substitute_food(
    stock: &mut ResourceInventory,
    items: &[ResourceType],
    mut missing: u64,
) -> HashMap<ResourceType, u32> {
    let mut withdrawals = HashMap::new();
    for resource in items {
        if missing == 0 {
            break;
        }
        let amount = stock
            .amount(resource.clone())
            .min(inventory_quantity(missing));
        if amount > 0 && stock.consume(resource.clone(), amount) {
            missing -= u64::from(amount);
            withdrawals.insert(resource.clone(), amount);
        }
    }
    withdrawals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::ResourceType;
    use crate::test_support::material;
    use std::collections::HashMap;
    use std::sync::Arc;

    const CONFIGURED_NEEDS: &str = r#"
        food_units_per_person = 2
        food_items = ["grain", "bread", "fish"]

        [[tiers]]
        min_population = 1
        materials = { wood = 1 }
        diet = [
            { resource = "grain", percent = 25 },
            { resource = "bread", percent = 25 },
            { resource = "fish", percent = 50 },
        ]

        [[tiers]]
        min_population = 2
        materials = { wood = 2 }
        diet = [{ resource = "bread", percent = 100 }]
    "#;

    #[test]
    fn diet_rounding_preserves_total_and_breaks_equal_remainders_in_row_order() {
        let registry = Arc::new(
            NeedRegistry::from_toml(
                CONFIGURED_NEEDS,
                crate::domain::economy::ResourceCatalog::standard(),
            )
            .unwrap(),
        );
        assert!(registry.starting_food(0).is_empty());
        assert_eq!(
            registry.starting_food(1),
            HashMap::from([(material("grain"), 1), (material("fish"), 1)])
        );
        assert_eq!(
            registry.starting_food(2),
            HashMap::from([(material("bread"), 4)])
        );
        let manager = PopulationManager::new(2, registry);
        assert_eq!(
            manager.calculate_demand(),
            HashMap::from([(material("bread"), 4), (material("wood"), 6)])
        );
    }

    #[test]
    fn configured_needs_validate_food_class_diet_shares_unique_rows_and_tiers() {
        for invalid in [
            CONFIGURED_NEEDS.replace("\"grain\", \"bread\"", "\"beer\", \"bread\""),
            CONFIGURED_NEEDS.replace("percent = 50", "percent = 40"),
            CONFIGURED_NEEDS.replace("resource = \"grain\"", "resource = \"bread\""),
            CONFIGURED_NEEDS.replace("min_population = 2", "min_population = 1"),
        ] {
            assert!(matches!(
                NeedRegistry::from_toml(
                    &invalid,
                    crate::domain::economy::ResourceCatalog::standard()
                ),
                Err(NeedsConfigError::Invalid(_))
            ));
        }
    }

    #[test]
    fn reported_food_demand_stays_exact_above_the_inventory_quantity_limit() {
        let registry = Arc::new(
            NeedRegistry::from_toml(
                r#"
                food_units_per_person = 4294967295
                food_items = ["grain"]
                [[tiers]]
                min_population = 1
                diet = [{ resource = "grain", percent = 100 }]
            "#,
                crate::domain::economy::ResourceCatalog::standard(),
            )
            .unwrap(),
        );
        let manager = PopulationManager::new(2, registry);
        let demand = manager.calculate_demand();
        assert_eq!(demand[&material("grain")], u32::MAX);
        let result = manager.process_allocation(Some(&demand));
        assert_eq!(result.food_required, 2 * u64::from(u32::MAX));
        assert_eq!(result.food_eaten, u64::from(u32::MAX));
        assert_eq!(result.people_unfed, 1);
    }

    fn manager_with_tiers(population: u32, needs: &[&[(ResourceType, u32)]]) -> PopulationManager {
        let tiers = needs
            .iter()
            .map(|needs| NeedTier {
                min_population: 0,
                needs_per_capita: needs.iter().cloned().collect(),
            })
            .collect();
        PopulationManager::new(population, Arc::new(NeedRegistry::new(tiers)))
    }

    #[test]
    fn integer_needs_use_the_same_demand_for_requests_and_evaluation() {
        let manager = manager_with_tiers(4, &[&[(material("grain"), 3)]]);
        let requests = manager.generate_requests();
        assert_eq!(requests[0].demand[&material("grain")], 12);
        let result = manager.process_allocation(Some(&requests[0].demand));
        assert_eq!(result.satisfaction_percent, 1.0);
        assert!(result.missing_resources.is_empty());
    }

    #[test]
    fn repeated_resources_across_tiers_are_summed_in_the_same_units() {
        let manager =
            manager_with_tiers(3, &[&[(material("grain"), 1)], &[(material("grain"), 2)]]);
        let requests = manager.generate_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].demand[&material("grain")], 9);
    }

    #[test]
    fn excess_of_one_good_cannot_compensate_for_another_missing_good() {
        let manager = manager_with_tiers(1, &[&[(material("grain"), 1), (material("wood"), 1)]]);
        let result = manager.process_allocation(Some(&HashMap::from([
            (material("grain"), 2),
            (material("stone"), 100),
        ])));
        assert_eq!(result.satisfaction_percent, 0.5);
        assert_eq!(result.missing_resources, vec![material("wood")]);
    }

    #[test]
    fn one_unit_need_is_missing_without_its_grant() {
        let manager = manager_with_tiers(1, &[&[(material("grain"), 1)]]);
        let result = manager.process_allocation(None);
        assert_eq!(result.satisfaction_percent, 0.0);
        assert_eq!(result.missing_resources, vec![material("grain")]);
    }

    #[test]
    fn zero_needs_generate_no_requests() {
        let manager = manager_with_tiers(10, &[&[(material("grain"), 0)]]);
        assert!(manager.generate_requests().is_empty());
        assert_eq!(manager.process_allocation(None).satisfaction_percent, 1.0);
    }

    #[test]
    fn zero_population_has_no_needs_even_when_tier_zero_is_active() {
        let manager = manager_with_tiers(0, &[&[(material("grain"), 1)]]);
        assert!(manager.generate_requests().is_empty());
        let result = manager.process_allocation(Some(&HashMap::from([(material("grain"), 100)])));
        assert_eq!(result.satisfaction_percent, 1.0);
        assert!(result.missing_resources.is_empty());
    }

    #[test]
    fn complete_and_excess_grants_always_produce_full_satisfaction() {
        for population in 1..=5 {
            for rate in 0..=5 {
                let manager = manager_with_tiers(
                    population,
                    &[&[(material("grain"), rate), (material("wood"), 1)]],
                );
                let requests = manager.generate_requests();
                let demand = &requests[0].demand;
                let complete = manager.process_allocation(Some(demand));
                assert_eq!(complete.satisfaction_percent, 1.0);
                assert!(complete.missing_resources.is_empty());
                let excess: HashMap<_, _> = demand
                    .iter()
                    .map(|(resource, &amount)| (resource.clone(), amount + 1))
                    .collect();
                assert_eq!(
                    manager
                        .process_allocation(Some(&excess))
                        .satisfaction_percent,
                    1.0
                );
            }
        }
    }

    #[test]
    fn integer_demands_keep_large_quantities_exact() {
        for population in [16_777_219, u32::MAX] {
            let manager = manager_with_tiers(
                population,
                &[&[(material("grain"), 1), (material("wood"), 1)]],
            );
            let requests = manager.generate_requests();
            assert_eq!(requests[0].demand[&material("grain")], population);
            assert_eq!(requests[0].demand[&material("wood")], population);
            let result = manager.process_allocation(Some(&requests[0].demand));
            assert_eq!(result.satisfaction_percent, 1.0);
            assert!(result.missing_resources.is_empty());
        }
    }

    #[test]
    #[should_panic(expected = "per-capita demand exceeds u32 capacity")]
    fn overflowing_per_capita_sum_is_rejected() {
        manager_with_tiers(
            1,
            &[&[(material("grain"), u32::MAX)], &[(material("grain"), 1)]],
        )
        .generate_requests();
    }

    #[test]
    #[should_panic(expected = "population demand exceeds u32 capacity")]
    fn overflowing_population_demand_is_rejected() {
        manager_with_tiers(2, &[&[(material("grain"), u32::MAX)]]).generate_requests();
    }

    fn create_test_registry() -> Arc<NeedRegistry> {
        let mut t1_needs = HashMap::new();
        t1_needs.insert(material("grain"), 1);

        let mut t2_needs = HashMap::new();
        t2_needs.insert(material("wood"), 1);

        let tiers = vec![
            NeedTier {
                min_population: 1,
                needs_per_capita: t1_needs,
            },
            NeedTier {
                min_population: 50,
                needs_per_capita: t2_needs,
            },
        ];

        Arc::new(NeedRegistry::new(tiers))
    }

    #[test]
    fn test_registry_active_tiers_accumulation() {
        let registry = create_test_registry();

        let active_for_20: Vec<_> = registry.get_active_tiers(20).collect();
        assert_eq!(active_for_20.len(), 1);
        assert_eq!(active_for_20[0].min_population, 1);

        let active_for_60: Vec<_> = registry.get_active_tiers(60).collect();
        assert_eq!(active_for_60.len(), 2, "Both tiers should be active.");
    }

    #[test]
    fn test_generate_requests_cumulative() {
        let registry = create_test_registry();
        let manager = PopulationManager::new(100, registry);

        let requests = manager.generate_requests();

        assert_eq!(
            requests.len(),
            1,
            "All active tiers should generate one combined population request."
        );

        // Verify the Grain request
        let grain_req = requests
            .iter()
            .find(|r| r.demand.contains_key(&material("grain")))
            .unwrap();
        assert_eq!(
            *grain_req.demand.get(&material("grain")).unwrap(),
            100,
            "Grain should be requested for first tier."
        );

        // Verify the Wood request
        let wood_req = requests
            .iter()
            .find(|r| r.demand.contains_key(&material("wood")))
            .unwrap();
        assert_eq!(
            *wood_req.demand.get(&material("wood")).unwrap(),
            100,
            "Wood should be requested for second tier."
        );
    }

    #[test]
    fn test_process_allocation_full_satisfaction() {
        let registry = create_test_registry();
        let manager = PopulationManager::new(10, registry); // Needs 10 Grain

        // Mock the AllocationEngine granting exactly what was requested as u32
        let mut granted = HashMap::new();
        granted.insert(material("grain"), 10);

        let result = manager.process_allocation(Some(&granted));

        assert_eq!(result.satisfaction_percent, 1.0);
        assert!(result.missing_resources.is_empty());
    }

    #[test]
    fn test_process_allocation_partial_satisfaction_smooth_gradient() {
        let registry = create_test_registry();
        let manager = PopulationManager::new(100, registry);
        // Total needs across all tiers: 100 Grain + 100 Wood = 200 units

        // Mock the AllocationEngine granting partial resources
        let mut granted = HashMap::new();
        granted.insert(material("grain"), 100);
        // Wood is completely missing (0 granted)

        let result = manager.process_allocation(Some(&granted));

        // 100 units granted out of 200 needed = 50% satisfaction
        assert_eq!(result.satisfaction_percent, 0.5);
        assert_eq!(result.missing_resources, vec![material("wood")]);

        // Grain is fully satisfied, while wood is missing.
        assert!(result.missing_resources.contains(&material("wood")));
    }

    #[test]
    fn test_process_allocation_zero_population() {
        let registry = create_test_registry();
        let manager = PopulationManager::new(0, registry);

        let granted = HashMap::new();

        let result = manager.process_allocation(Some(&granted));

        assert_eq!(
            result.satisfaction_percent, 1.0,
            "Ghost towns have no demands and are always satisfied."
        );
        assert!(result.missing_resources.is_empty());
    }
}
