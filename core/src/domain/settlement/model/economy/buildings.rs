use crate::domain::buildings::Building;
use crate::domain::buildings::definition::BuildingDefinition;
use crate::domain::buildings::productivity::{
    BuildingProductivityCalculator, LocationProductivity, ProductionModifier,
    ProductionModifierStrategy, TravelCostSource,
};
use crate::domain::economy::ResourceType;
use crate::domain::economy::allocation::{AllocationRecipient, AllocationResult, DemandRequest};
use crate::domain::ids::BuildingId;
use crate::domain::spatial::ConstructionSpace;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct BuildingManager {
    pub(in crate::domain::settlement) buildings: Vec<Building>,
    productivity_calculator: Option<BuildingProductivityCalculator>,
    production_modifier_strategies: Vec<Box<dyn ProductionModifierStrategy>>,
}

#[derive(Default)]
pub(crate) struct BuildingRequests {
    pub demands: Vec<DemandRequest>,
    pub recipes: HashMap<BuildingId, usize>,
}

impl BuildingManager {
    pub fn new() -> Self {
        Self {
            buildings: Vec::new(),
            productivity_calculator: None,
            production_modifier_strategies: Vec::new(),
        }
    }

    pub fn construct_building(
        &mut self,
        mut building: Building,
        source: &mut impl TravelCostSource,
    ) -> Result<(), BuildingId> {
        if self.contains(building.instance_id()) {
            return Err(building.instance_id());
        }
        building.set_location_productivity(self.productivity_at(building.position(), source));
        self.buildings.push(building);
        Ok(())
    }

    pub fn productivity_calculator(&self) -> Option<BuildingProductivityCalculator> {
        self.productivity_calculator
    }

    pub(in crate::domain::settlement) fn use_capital(
        &mut self,
        capital: crate::domain::spatial::Position,
    ) {
        self.productivity_calculator = self
            .productivity_calculator
            .map(|calculator| BuildingProductivityCalculator::new(capital, calculator.policy()));
    }

    pub fn set_productivity_calculator(
        &mut self,
        calculator: Option<BuildingProductivityCalculator>,
        source: &mut impl TravelCostSource,
    ) {
        self.productivity_calculator = calculator;
        self.refresh_productivity(source);
    }

    pub fn refresh_productivity(&mut self, source: &mut impl TravelCostSource) {
        let calculator = self.productivity_calculator;
        for building in &mut self.buildings {
            let productivity = calculator.map_or(LocationProductivity::FULL, |calculator| {
                calculator.multiplier_at(building.position(), source)
            });
            building.set_location_productivity(productivity);
        }
    }

    pub(in crate::domain::settlement) fn add_production_modifier_strategy(
        &mut self,
        strategy: impl ProductionModifierStrategy + 'static,
    ) {
        self.production_modifier_strategies.push(Box::new(strategy));
    }

    pub(in crate::domain::settlement) fn refresh_production_biomes(
        &mut self,
        space: &impl ConstructionSpace,
    ) {
        let uses_strategies = !self.production_modifier_strategies.is_empty();
        for building in &mut self.buildings {
            if uses_strategies || building.definition().is_biome_restricted() {
                building.production_biome = space.production_biome(building.position());
            }
        }
    }

    fn productivity_at(
        &self,
        position: crate::domain::spatial::Position,
        source: &mut impl TravelCostSource,
    ) -> LocationProductivity {
        self.productivity_calculator
            .map_or(LocationProductivity::FULL, |calculator| {
                calculator.multiplier_at(position, source)
            })
    }

    pub fn get(&self, id: BuildingId) -> Option<&Building> {
        self.buildings
            .iter()
            .find(|building| building.instance_id() == id)
    }

    pub fn get_mut(&mut self, id: BuildingId) -> Option<&mut Building> {
        self.buildings
            .iter_mut()
            .find(|building| building.instance_id() == id)
    }

    pub fn contains(&self, id: BuildingId) -> bool {
        self.get(id).is_some()
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
        if let Some(building) = self.get_mut(target_id) {
            building.upgrade(new_def);
            true
        } else {
            false
        }
    }

    pub(crate) fn generate_requests(
        &self,
        can_operate: impl Fn(&Building) -> bool,
        available: impl Fn(&ResourceType) -> u32,
    ) -> BuildingRequests {
        let mut plan = BuildingRequests::default();
        let mut eligible: Vec<_> = self
            .buildings
            .iter()
            .filter(|building| can_operate(building))
            .collect();
        eligible.sort_unstable_by_key(|building| building.instance_id());
        let mut alternative_requests = HashMap::<ResourceType, u32>::new();
        for building in eligible {
            let id = building.instance_id();
            let demand = if let Some(production) = building.definition().cycle_production() {
                let has_alternatives = production.recipes.len() > 1;
                let selection = building.preferred_cycle_recipe(|resource| {
                    let stock = available(resource);
                    if has_alternatives {
                        stock - alternative_requests.get(resource).copied().unwrap_or(0)
                    } else {
                        stock
                    }
                });
                if let Some((recipe, _)) = selection {
                    plan.recipes.insert(id, recipe);
                }
                let demand = building.calculate_cycle_demand(selection);
                if has_alternatives {
                    for (resource, &amount) in &demand {
                        if resource != &ResourceType::Labour {
                            *alternative_requests.entry(resource.clone()).or_default() += amount;
                        }
                    }
                }
                demand
            } else {
                building.calculate_demand()
            };
            if !demand.is_empty() {
                plan.demands.push(DemandRequest {
                    recipient: AllocationRecipient::Building(id),
                    demand,
                });
            }
        }
        plan
    }

    pub fn process_allocations(
        &mut self,
        allocations: &AllocationResult,
        recipes: &HashMap<BuildingId, usize>,
        turn_modifier: Option<&ProductionModifier>,
    ) -> (HashMap<ResourceType, u64>, HashMap<ResourceType, u64>) {
        let mut all_produced = HashMap::new();
        let mut all_returned = HashMap::new();

        for building in self.buildings.iter_mut() {
            let id = building.instance_id();
            let grants = allocations.get(&AllocationRecipient::Building(id));
            let update_result = building.resolve_planned_allocation(
                grants,
                &self.production_modifier_strategies,
                recipes.get(&id).copied(),
                turn_modifier,
            );

            for (res, amount) in update_result.produced {
                let total = all_produced.entry(res).or_insert(0_u64);
                *total = total.saturating_add(amount);
            }
            for (res, amount) in update_result.unused {
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
    use crate::domain::world::Map;
    use crate::domain::world::pathfinding::path_finder::PathFinder;
    use crate::domain::world::settlement_environment::AStarTravelCosts;
    use crate::test_support::material;

    fn test_map() -> Map {
        Map::new(
            1,
            1,
            vec![crate::domain::world::terrain::area::Area::new(0.5, 0.5)],
            vec![0],
        )
    }
    fn test_finder() -> PathFinder {
        PathFinder::new(1, 1)
    }

    use super::*;
    use crate::domain::buildings::Building;
    use crate::domain::buildings::definition::{BuildingBehavior, BuildingDefinition};
    use crate::domain::economy::ResourceType;
    use crate::domain::spatial::Position;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn duplicate_building_ids_are_rejected() {
        let mut manager = BuildingManager::new();
        let def = create_test_definition("test", 1, vec![]);
        manager
            .construct_building(
                create_active_building(7, def.clone()),
                &mut AStarTravelCosts::new(&test_map(), &mut test_finder()),
            )
            .unwrap();
        let duplicate = create_active_building(7, def);
        assert_eq!(
            manager.construct_building(
                duplicate,
                &mut AStarTravelCosts::new(&test_map(), &mut test_finder())
            ),
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
        Arc::new(
            BuildingDefinition::new(
                id.to_string(),
                "Test Building".to_string(),
                1,
                max_workers,
                behaviors,
                Default::default(),
            )
            .unwrap(),
        )
    }

    fn create_active_building(instance_id: u32, def: Arc<BuildingDefinition>) -> Building {
        let mut building = Building::new(BuildingId(instance_id), Position { x: 0, y: 0 }, def);
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
            .construct_building(
                first,
                &mut AStarTravelCosts::new(&test_map(), &mut test_finder()),
            )
            .unwrap();
        manager
            .construct_building(
                second,
                &mut AStarTravelCosts::new(&test_map(), &mut test_finder()),
            )
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
                &mut AStarTravelCosts::new(&test_map(), &mut test_finder()),
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
                &mut AStarTravelCosts::new(&test_map(), &mut test_finder()),
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
                &mut AStarTravelCosts::new(&test_map(), &mut test_finder()),
            )
            .unwrap();

        // Building 2 wants 10 Labour + 10 Wood (via Production behavior).
        let prod_behavior = BuildingBehavior::Production {
            inputs: vec![(material("wood"), 10)],
            outputs: vec![(material("stone"), 20)],
        };
        let def2 = create_test_definition("lumber", 10, vec![prod_behavior]);
        manager
            .construct_building(
                create_active_building(2, def2),
                &mut AStarTravelCosts::new(&test_map(), &mut test_finder()),
            )
            .unwrap();

        // Act
        let requests = manager.generate_requests(|_| true, |_| 0).demands;

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
        assert!(!req1.demand.contains_key(&material("wood")));

        // Verify Building 2 demand
        let req2 = requests
            .iter()
            .find(|r| r.recipient == AllocationRecipient::Building(BuildingId(2)))
            .unwrap();
        assert_eq!(*req2.demand.get(&ResourceType::Labour).unwrap(), 10);
        assert_eq!(*req2.demand.get(&material("wood")).unwrap(), 10);
    }

    #[test]
    fn test_process_allocations_aggregates_production_and_unused() {
        // Arrange
        let mut manager = BuildingManager::new();

        let behavior = BuildingBehavior::Production {
            inputs: vec![(material("wood"), 10)],
            outputs: vec![(material("stone"), 20)],
        };

        let def = create_test_definition("stone_maker", 10, vec![behavior]);

        // Add two identical buildings
        manager
            .construct_building(
                create_active_building(1, Arc::clone(&def)),
                &mut AStarTravelCosts::new(&test_map(), &mut test_finder()),
            )
            .unwrap();
        manager
            .construct_building(
                create_active_building(2, def),
                &mut AStarTravelCosts::new(&test_map(), &mut test_finder()),
            )
            .unwrap();

        // Setup AllocationResult
        let mut allocations: AllocationResult = HashMap::new();

        // Building 1 gets exactly what it needs
        let mut b1_grants = HashMap::new();
        b1_grants.insert(ResourceType::Labour, 10);
        b1_grants.insert(material("wood"), 10);
        allocations.insert(AllocationRecipient::Building(BuildingId(1)), b1_grants);

        // Building 2 gets full labour, but EXTRA wood (15 instead of 10)
        let mut b2_grants = HashMap::new();
        b2_grants.insert(ResourceType::Labour, 10);
        b2_grants.insert(material("wood"), 15);
        allocations.insert(AllocationRecipient::Building(BuildingId(2)), b2_grants);

        // Act
        let (produced, returned) = manager.process_allocations(&allocations, &HashMap::new(), None);

        // Assert
        // Both buildings worked at 100% capacity (10 labour, enough wood).
        // Building 1 makes 20 stone, Building 2 makes 20 stone -> Total 40 stone.
        assert_eq!(
            *produced.get(&material("stone")).unwrap_or(&0),
            40,
            "Should aggregate stone production from both buildings"
        );

        // Building 1 used all 10 wood (0 unused).
        // Building 2 received 15 wood, used 10, returning 5. -> Total 5 unused wood.
        assert_eq!(
            *returned.get(&material("wood")).unwrap_or(&0),
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
