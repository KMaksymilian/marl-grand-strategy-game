use crate::domain::economy::resource::{ResourceInventory, ResourceType};
use std::collections::HashMap;

#[derive(Debug)]
pub struct SettlementInventoryManager {
    pub main: ResourceInventory,
    pub pending_production: ResourceInventory,
}

impl SettlementInventoryManager {
    pub fn new(capacity: u32) -> Self {
        Self {
            main: ResourceInventory::new(capacity),
            pending_production: ResourceInventory::new(capacity),
        }
    }

    /// Reset Labour amount at the start of turn
    pub fn refresh_labor_capacity(&mut self, population: u32) {
        self.main.resources.insert(ResourceType::Labour, population);
    }

    /// Produced goods are stored in buffer
    pub fn process_production_results(
        &mut self,
        produced: HashMap<ResourceType, u32>,
        returned: HashMap<ResourceType, u32>,
    ) {
        for (res, amount) in returned {
            self.main.add(res, amount);
        }
        for (res, amount) in produced {
            self.pending_production.add(res, amount);
        }
    }

    /// Move goods from buffer to inventory
    pub fn commit_pending_production(&mut self) {
        for (res, amount) in self.pending_production.resources.drain() {
            self.main.add(res, amount);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::resource::ResourceType;
    use std::collections::HashMap;

    #[test]
    fn test_new_initializes_with_correct_capacities() {
        // Arrange & Act
        let manager = SettlementInventoryManager::new(150);

        // Assert
        assert_eq!(
            manager.main.max_capacity, 150,
            "Main inventory should have correct capacity"
        );
        assert_eq!(
            manager.pending_production.max_capacity, 150,
            "Pending inventory should have correct capacity"
        );
        assert_eq!(manager.main.total_amount(), 0);
        assert_eq!(manager.pending_production.total_amount(), 0);
    }

    #[test]
    fn test_refresh_labor_capacity_inserts_and_overwrites() {
        // Arrange
        let mut manager = SettlementInventoryManager::new(100);

        // Act 1: Initial set
        manager.refresh_labor_capacity(50);

        // Assert 1
        assert_eq!(
            *manager.main.resources.get(&ResourceType::Labour).unwrap(),
            50,
            "Labour should be set to 50"
        );
        assert_eq!(
            manager.main.total_amount(),
            0,
            "Labour should not count towards total capacity limits"
        );

        // Act 2: Overwrite on next turn
        manager.refresh_labor_capacity(60);

        // Assert 2
        assert_eq!(
            *manager.main.resources.get(&ResourceType::Labour).unwrap(),
            60,
            "Labour should be overwritten to 60, not added"
        );
    }

    #[test]
    fn test_process_production_results_routes_correctly() {
        // Arrange
        let mut manager = SettlementInventoryManager::new(100);

        let mut produced = HashMap::new();
        produced.insert(ResourceType::Wood, 30);

        let mut returned = HashMap::new();
        returned.insert(ResourceType::Stone, 20);

        // Act
        manager.process_production_results(produced, returned);

        // Assert
        // Returned items go straight back to main
        assert_eq!(manager.main.total_amount(), 20);
        assert_eq!(
            *manager.main.resources.get(&ResourceType::Stone).unwrap(),
            20
        );

        // Produced items go to pending buffer
        assert_eq!(manager.pending_production.total_amount(), 30);
        assert_eq!(
            *manager
                .pending_production
                .resources
                .get(&ResourceType::Wood)
                .unwrap(),
            30
        );
    }

    #[test]
    fn test_commit_pending_production_moves_resources_to_main() {
        // Arrange
        let mut manager = SettlementInventoryManager::new(100);

        // Add some initial resources to main
        manager.main.add(ResourceType::Stone, 10);

        // Put resources in pending
        let mut produced = HashMap::new();
        produced.insert(ResourceType::Wood, 40);
        manager.process_production_results(produced, HashMap::new());

        // Act
        manager.commit_pending_production();

        // Assert
        assert!(
            manager.pending_production.resources.is_empty(),
            "Pending production should be drained completely"
        );

        assert_eq!(
            manager.main.total_amount(),
            50, // 10 stone + 40 wood
            "Main inventory should now contain both existing and committed resources"
        );
        assert_eq!(
            *manager.main.resources.get(&ResourceType::Wood).unwrap(),
            40
        );
        assert_eq!(
            *manager.main.resources.get(&ResourceType::Stone).unwrap(),
            10
        );
    }

    #[test]
    fn test_commit_pending_production_respects_main_capacity() {
        // Arrange
        let mut manager = SettlementInventoryManager::new(50);

        // Fill main inventory almost to the brim (40/50)
        manager.main.add(ResourceType::Stone, 40);

        // Put 30 wood in pending
        let mut produced = HashMap::new();
        produced.insert(ResourceType::Wood, 30);
        manager.process_production_results(produced, HashMap::new());

        // Act
        manager.commit_pending_production();

        // Assert
        assert!(
            manager.pending_production.resources.is_empty(),
            "Pending production is always drained, even if main can't fit everything"
        );

        assert_eq!(
            manager.main.total_amount(),
            50,
            "Main inventory should be capped at its max capacity (50)"
        );

        // Only 10 wood should have made it in (50 max - 40 stone = 10 space left)
        assert_eq!(
            *manager.main.resources.get(&ResourceType::Wood).unwrap(),
            10,
            "Only the amount that fits should be transferred from pending"
        );
    }
}
