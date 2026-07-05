use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Wood,
    Stone,
    Grain,
    // work in progress
}

#[derive(Debug, Default)]
pub struct ResourceInventory {
    pub resources: HashMap<ResourceType, f32>,
    pub max_capacity: f32,
}

impl ResourceInventory {
    pub fn new(max_capacity: f32) -> Self {
        Self {
            resources: HashMap::new(),
            max_capacity,
        }
    }
    pub fn total_amount(&self) -> f32 {
        self.resources.values().sum()
    }

    pub fn add(&mut self, res_type: ResourceType, amount: f32) {
        let available_space = self.max_capacity - self.total_amount();

        if available_space <= 0.0 {
            return;
        }

        let amount_to_add = amount.min(available_space);

        *self.resources.entry(res_type).or_insert(0.0) += amount_to_add;
    }

    pub fn consume(&mut self, res_type: ResourceType, amount: f32) -> bool {
        if let Some(current) = self.resources.get_mut(&res_type)
            && *current >= amount
        {
            *current -= amount;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_inventory() {
        let inv = ResourceInventory::new(100.0);
        assert_eq!(inv.max_capacity, 100.0);
        assert_eq!(inv.total_amount(), 0.0);
        assert!(inv.resources.is_empty());
    }

    #[test]
    fn test_add_within_capacity() {
        let mut inv = ResourceInventory::new(100.0);
        inv.add(ResourceType::Wood, 40.0);

        assert_eq!(inv.total_amount(), 40.0);
        assert_eq!(*inv.resources.get(&ResourceType::Wood).unwrap(), 40.0);
    }

    #[test]
    fn test_add_exceeding_capacity() {
        let mut inv = ResourceInventory::new(50.0);
        inv.add(ResourceType::Stone, 60.0);

        assert_eq!(inv.total_amount(), 50.0);
        assert_eq!(*inv.resources.get(&ResourceType::Stone).unwrap(), 50.0);
    }

    #[test]
    fn test_add_multiple_resources_with_overflow() {
        let mut inv = ResourceInventory::new(100.0);
        inv.add(ResourceType::Wood, 30.0);
        inv.add(ResourceType::Grain, 50.0);

        assert_eq!(inv.total_amount(), 80.0);

        inv.add(ResourceType::Stone, 30.0);

        assert_eq!(inv.total_amount(), 100.0);
        assert_eq!(*inv.resources.get(&ResourceType::Stone).unwrap(), 20.0);
    }

    #[test]
    fn test_add_when_already_full() {
        let mut inv = ResourceInventory::new(100.0);
        inv.add(ResourceType::Wood, 100.0);

        inv.add(ResourceType::Grain, 10.0);

        assert_eq!(inv.total_amount(), 100.0);
        assert_eq!(inv.resources.get(&ResourceType::Grain), None);
    }

    #[test]
    fn test_consume_success() {
        let mut inv = ResourceInventory::new(100.0);
        inv.add(ResourceType::Grain, 50.0);

        let result = inv.consume(ResourceType::Grain, 20.0);

        assert!(result);
        assert_eq!(inv.total_amount(), 30.0);
        assert_eq!(*inv.resources.get(&ResourceType::Grain).unwrap(), 30.0);
    }

    #[test]
    fn test_consume_insufficient_amount() {
        let mut inv = ResourceInventory::new(100.0);
        inv.add(ResourceType::Wood, 20.0);

        let result = inv.consume(ResourceType::Wood, 30.0);

        assert!(!result);
        assert_eq!(inv.total_amount(), 20.0);
    }

    #[test]
    fn test_consume_nonexistent_resource() {
        let mut inv = ResourceInventory::new(100.0);

        let result = inv.consume(ResourceType::Stone, 10.0);

        assert!(!result);
    }
}
