use crate::domain::economy::resource::ResourceType;
use crate::domain::economy::resource_allocation::{AllocationRecipient, DemandRequest};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct ConsumptionResult {
    pub satisfaction_percent: f32,
    pub missing_resources: Vec<ResourceType>,
}

#[derive(Debug, Clone)]
pub struct NeedTier {
    pub min_population: u32,
    pub needs_per_capita: HashMap<ResourceType, u32>,
}

#[derive(Debug, Default)]
pub struct NeedRegistry {
    tiers: Vec<NeedTier>,
}

impl NeedRegistry {
    pub fn new(mut tiers: Vec<NeedTier>) -> Self {
        tiers.sort_by_key(|b| std::cmp::Reverse(b.min_population));
        Self { tiers }
    }

    pub fn get_active_tiers(&self, population: u32) -> impl Iterator<Item = &NeedTier> {
        self.tiers
            .iter()
            .filter(move |tier| population >= tier.min_population)
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
    fn calculate_demand(&self) -> HashMap<ResourceType, u32> {
        if self.population == 0 {
            return HashMap::new();
        }
        let mut per_capita = HashMap::new();
        for tier in self.registry.get_active_tiers(self.population) {
            for (&resource, &amount) in &tier.needs_per_capita {
                let total = per_capita.entry(resource).or_insert(0_u32);
                *total = total
                    .checked_add(amount)
                    .expect("per-capita demand exceeds u32 capacity");
            }
        }
        per_capita
            .into_iter()
            .filter_map(|(resource, amount)| {
                let required = amount
                    .checked_mul(self.population)
                    .expect("population demand exceeds u32 capacity");
                (required > 0).then_some((resource, required))
            })
            .collect()
    }

    pub fn process_allocation(
        &self,
        granted_resources: Option<&HashMap<ResourceType, u32>>,
    ) -> ConsumptionResult {
        let mut total_demanded = 0_u64;
        let mut total_satisfied = 0_u64;
        let mut missing_resources = Vec::new();

        let empty_grants = HashMap::new();
        let granted = granted_resources.unwrap_or(&empty_grants);

        for (res, required) in self.calculate_demand() {
            total_demanded += u64::from(required);
            let received = granted.get(&res).copied().unwrap_or(0);
            total_satisfied += u64::from(received.min(required));
            if received < required {
                missing_resources.push(res);
            }
        }
        missing_resources.sort_unstable();

        let satisfaction_percent = if total_demanded > 0 {
            (total_satisfied as f64 / total_demanded as f64) as f32
        } else {
            1.0
        };

        ConsumptionResult {
            satisfaction_percent,
            missing_resources,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::resource::ResourceType;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn manager_with_tiers(population: u32, needs: &[&[(ResourceType, u32)]]) -> PopulationManager {
        let tiers = needs
            .iter()
            .map(|needs| NeedTier {
                min_population: 0,
                needs_per_capita: needs.iter().copied().collect(),
            })
            .collect();
        PopulationManager::new(population, Arc::new(NeedRegistry::new(tiers)))
    }

    #[test]
    fn integer_needs_use_the_same_demand_for_requests_and_evaluation() {
        let manager = manager_with_tiers(4, &[&[(ResourceType::Grain, 3)]]);
        let requests = manager.generate_requests();
        assert_eq!(requests[0].demand[&ResourceType::Grain], 12);
        let result = manager.process_allocation(Some(&requests[0].demand));
        assert_eq!(result.satisfaction_percent, 1.0);
        assert!(result.missing_resources.is_empty());
    }

    #[test]
    fn repeated_resources_across_tiers_are_summed_in_the_same_units() {
        let manager = manager_with_tiers(
            3,
            &[&[(ResourceType::Grain, 1)], &[(ResourceType::Grain, 2)]],
        );
        let requests = manager.generate_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].demand[&ResourceType::Grain], 9);
    }

    #[test]
    fn excess_of_one_good_cannot_compensate_for_another_missing_good() {
        let manager =
            manager_with_tiers(1, &[&[(ResourceType::Grain, 1), (ResourceType::Wood, 1)]]);
        let result = manager.process_allocation(Some(&HashMap::from([
            (ResourceType::Grain, 2),
            (ResourceType::Stone, 100),
        ])));
        assert_eq!(result.satisfaction_percent, 0.5);
        assert_eq!(result.missing_resources, vec![ResourceType::Wood]);
    }

    #[test]
    fn one_unit_need_is_missing_without_its_grant() {
        let manager = manager_with_tiers(1, &[&[(ResourceType::Grain, 1)]]);
        let result = manager.process_allocation(None);
        assert_eq!(result.satisfaction_percent, 0.0);
        assert_eq!(result.missing_resources, vec![ResourceType::Grain]);
    }

    #[test]
    fn zero_needs_generate_no_requests() {
        let manager = manager_with_tiers(10, &[&[(ResourceType::Grain, 0)]]);
        assert!(manager.generate_requests().is_empty());
        assert_eq!(manager.process_allocation(None).satisfaction_percent, 1.0);
    }

    #[test]
    fn zero_population_has_no_needs_even_when_tier_zero_is_active() {
        let manager = manager_with_tiers(0, &[&[(ResourceType::Grain, 1)]]);
        assert!(manager.generate_requests().is_empty());
        let result = manager.process_allocation(Some(&HashMap::from([(ResourceType::Grain, 100)])));
        assert_eq!(result.satisfaction_percent, 1.0);
        assert!(result.missing_resources.is_empty());
    }

    #[test]
    fn complete_and_excess_grants_always_produce_full_satisfaction() {
        for population in 1..=5 {
            for rate in 0..=5 {
                let manager = manager_with_tiers(
                    population,
                    &[&[(ResourceType::Grain, rate), (ResourceType::Wood, 1)]],
                );
                let requests = manager.generate_requests();
                let demand = &requests[0].demand;
                let complete = manager.process_allocation(Some(demand));
                assert_eq!(complete.satisfaction_percent, 1.0);
                assert!(complete.missing_resources.is_empty());
                let excess: HashMap<_, _> = demand
                    .iter()
                    .map(|(&resource, &amount)| (resource, amount + 1))
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
                &[&[(ResourceType::Grain, 1), (ResourceType::Wood, 1)]],
            );
            let requests = manager.generate_requests();
            assert_eq!(requests[0].demand[&ResourceType::Grain], population);
            assert_eq!(requests[0].demand[&ResourceType::Wood], population);
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
            &[
                &[(ResourceType::Grain, u32::MAX)],
                &[(ResourceType::Grain, 1)],
            ],
        )
        .generate_requests();
    }

    #[test]
    #[should_panic(expected = "population demand exceeds u32 capacity")]
    fn overflowing_population_demand_is_rejected() {
        manager_with_tiers(2, &[&[(ResourceType::Grain, u32::MAX)]]).generate_requests();
    }

    fn create_test_registry() -> Arc<NeedRegistry> {
        let mut t1_needs = HashMap::new();
        t1_needs.insert(ResourceType::Grain, 1);

        let mut t2_needs = HashMap::new();
        t2_needs.insert(ResourceType::Wood, 1);

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
            .find(|r| r.demand.contains_key(&ResourceType::Grain))
            .unwrap();
        assert_eq!(
            *grain_req.demand.get(&ResourceType::Grain).unwrap(),
            100,
            "Grain should be requested for first tier."
        );

        // Verify the Wood request
        let wood_req = requests
            .iter()
            .find(|r| r.demand.contains_key(&ResourceType::Wood))
            .unwrap();
        assert_eq!(
            *wood_req.demand.get(&ResourceType::Wood).unwrap(),
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
        granted.insert(ResourceType::Grain, 10);

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
        granted.insert(ResourceType::Grain, 100);
        // Wood is completely missing (0 granted)

        let result = manager.process_allocation(Some(&granted));

        // 100 units granted out of 200 needed = 50% satisfaction
        assert_eq!(result.satisfaction_percent, 0.5);
        assert_eq!(result.missing_resources, vec![ResourceType::Wood]);

        // Grain is fully satisfied, while wood is missing.
        assert!(result.missing_resources.contains(&ResourceType::Wood));
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
