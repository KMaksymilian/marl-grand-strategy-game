use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Wood,
    Stone,
    Grain,
    Labour
    // work in progress
}

#[derive(Debug, Default)]
pub struct ResourceInventory {
    pub resources: HashMap<ResourceType, u32>,
    pub max_capacity: u32,
}

impl ResourceInventory {
    pub fn new(max_capacity: u32) -> Self {
        Self {
            resources: HashMap::new(),
            max_capacity,
        }
    }
    pub fn total_amount(&self) -> u32 {
        self.resources.values().sum()
    }

    pub fn add(&mut self, res_type: ResourceType, amount: u32) {
        let available_space = self.max_capacity - self.total_amount();

        if available_space <= 0 {
            return;
        }

        let amount_to_add = amount.min(available_space);

        *self.resources.entry(res_type).or_insert(0) += amount_to_add;
    }

    pub fn consume(&mut self, res_type: ResourceType, amount: u32) -> bool {
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
        let inv = ResourceInventory::new(100);
        assert_eq!(inv.max_capacity, 100);
        assert_eq!(inv.total_amount(), 0);
        assert!(inv.resources.is_empty());
    }

    #[test]
    fn test_add_within_capacity() {
        let mut inv = ResourceInventory::new(100);
        inv.add(ResourceType::Wood, 40);

        assert_eq!(inv.total_amount(), 40);
        assert_eq!(*inv.resources.get(&ResourceType::Wood).unwrap(), 40);
    }

    #[test]
    fn test_add_exceeding_capacity() {
        let mut inv = ResourceInventory::new(50);
        inv.add(ResourceType::Stone, 60);

        assert_eq!(inv.total_amount(), 50);
        assert_eq!(*inv.resources.get(&ResourceType::Stone).unwrap(), 50);
    }

    #[test]
    fn test_add_multiple_resources_with_overflow() {
        let mut inv = ResourceInventory::new(100);
        inv.add(ResourceType::Wood, 30);
        inv.add(ResourceType::Grain, 50);

        assert_eq!(inv.total_amount(), 80);

        inv.add(ResourceType::Stone, 30);

        assert_eq!(inv.total_amount(), 100);
        assert_eq!(*inv.resources.get(&ResourceType::Stone).unwrap(), 20);
    }

    #[test]
    fn test_add_when_already_full() {
        let mut inv = ResourceInventory::new(100);
        inv.add(ResourceType::Wood, 100);

        inv.add(ResourceType::Grain, 10);

        assert_eq!(inv.total_amount(), 100);
        assert_eq!(inv.resources.get(&ResourceType::Grain), None);
    }

    #[test]
    fn test_consume_success() {
        let mut inv = ResourceInventory::new(100);
        inv.add(ResourceType::Grain, 50);

        let result = inv.consume(ResourceType::Grain, 20);

        assert!(result);
        assert_eq!(inv.total_amount(), 30);
        assert_eq!(*inv.resources.get(&ResourceType::Grain).unwrap(), 30);
    }

    #[test]
    fn test_consume_insufficient_amount() {
        let mut inv = ResourceInventory::new(100);
        inv.add(ResourceType::Wood, 20);

        let result = inv.consume(ResourceType::Wood, 30);

        assert!(!result);
        assert_eq!(inv.total_amount(), 20);
    }

    #[test]
    fn test_consume_nonexistent_resource() {
        let mut inv = ResourceInventory::new(100);

        let result = inv.consume(ResourceType::Stone, 10);

        assert!(!result);
    }
}
