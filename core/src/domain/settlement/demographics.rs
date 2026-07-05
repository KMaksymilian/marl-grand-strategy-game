use crate::domain::economy::resource::ResourceType;
use crate::domain::economy::resource_allocation::{DemandRequest, Priority};
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
    pub priority: Priority,
    pub needs_per_capita: HashMap<ResourceType, f32>,
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
        let mut requests = Vec::new();

        if self.population == 0 {
            return requests;
        }

        for tier in self.registry.get_active_tiers(self.population) {
            let mut tier_demand = HashMap::new();

            for (res, amount) in &tier.needs_per_capita {
                tier_demand.insert(*res, amount * (self.population as f32));
            }

            requests.push(DemandRequest {
                entity_id: 0,
                priority: tier.priority,
                demand: tier_demand,
            });
        }

        requests
    }

    pub fn process_allocation(
        &self,
        granted_resources: Option<&HashMap<ResourceType, f32>>,
    ) -> ConsumptionResult {
        let mut total_demanded = 0.0;
        let mut total_consumed = 0.0;
        let mut missing_resources = Vec::new();

        let empty_grants = HashMap::new();
        let granted = granted_resources.unwrap_or(&empty_grants);

        let mut expected_demand = HashMap::new();
        for tier in self.registry.get_active_tiers(self.population) {
            for (res, amount) in &tier.needs_per_capita {
                *expected_demand.entry(*res).or_insert(0.0) += amount * (self.population as f32);
            }
        }

        for (res, required) in expected_demand {
            total_demanded += required;

            let consumed = granted.get(&res).copied().unwrap_or(0.0);
            total_consumed += consumed;

            if consumed < required - 0.0001 {
                missing_resources.push(res);
            }
        }

        let satisfaction_percent = if total_demanded > 0.0 {
            total_consumed / total_demanded
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
    use crate::domain::economy::resource_allocation::Priority;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn create_test_registry() -> Arc<NeedRegistry> {
        let mut t1_needs = HashMap::new();
        t1_needs.insert(ResourceType::Grain, 1.0);

        let mut t2_needs = HashMap::new();
        t2_needs.insert(ResourceType::Wood, 0.5);

        // We now assign a Priority to each NeedTier
        let tiers = vec![
            NeedTier {
                min_population: 1,
                priority: Priority::Survival,
                needs_per_capita: t1_needs,
            },
            NeedTier {
                min_population: 50,
                priority: Priority::Comfort,
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

        // In the new architecture, we generate requests instead of a flat demand map
        let requests = manager.generate_requests();

        assert_eq!(
            requests.len(),
            2,
            "Should generate two separate requests based on tiers."
        );

        // Verify the Survival tier request
        let survival_req = requests
            .iter()
            .find(|r| r.priority == Priority::Survival)
            .unwrap();
        assert_eq!(
            *survival_req.demand.get(&ResourceType::Grain).unwrap(),
            100.0,
            "Grain should be requested for Survival."
        );

        // Verify the Comfort tier request
        let comfort_req = requests
            .iter()
            .find(|r| r.priority == Priority::Comfort)
            .unwrap();
        assert_eq!(
            *comfort_req.demand.get(&ResourceType::Wood).unwrap(),
            50.0,
            "Wood should be requested for Comfort."
        );
    }

    #[test]
    fn test_process_allocation_full_satisfaction() {
        let registry = create_test_registry();
        let manager = PopulationManager::new(10, registry); // Needs 10 Grain

        // Mock the AllocationEngine granting exactly what was requested
        let mut granted = HashMap::new();
        granted.insert(ResourceType::Grain, 10.0);

        let result = manager.process_allocation(Some(&granted));

        assert_eq!(result.satisfaction_percent, 1.0);
        assert!(result.missing_resources.is_empty());
    }

    #[test]
    fn test_process_allocation_partial_satisfaction_smooth_gradient() {
        let registry = create_test_registry();
        let manager = PopulationManager::new(100, registry);
        // Total needs across all tiers: 100 Grain + 50 Wood = 150 units

        // Mock the AllocationEngine granting partial resources (bottleneck)
        let mut granted = HashMap::new();
        granted.insert(ResourceType::Grain, 75.0);
        // Wood is completely missing (0.0 granted)

        let result = manager.process_allocation(Some(&granted));

        // 75 units granted out of 150 units needed = 50% satisfaction
        assert_eq!(result.satisfaction_percent, 0.5);
        assert_eq!(result.missing_resources.len(), 2);

        // Both resources lacked their full expected amounts
        assert!(result.missing_resources.contains(&ResourceType::Grain));
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
