use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceType {
    Wood,
    Stone,
    Grain,
    Labour, // work in progress
}

#[derive(Debug, Default)]
pub struct ResourceInventory {
    pub resources: HashMap<ResourceType, u32>,
    max_capacity: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryAddResult {
    pub accepted: u32,
    pub rejected: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityError {
    pub requested: u32,
    pub stored: u64,
}

impl ResourceInventory {
    pub fn new(max_capacity: u32) -> Self {
        Self {
            resources: HashMap::new(),
            max_capacity,
        }
    }
    pub fn capacity(&self) -> u32 {
        self.max_capacity
    }

    /// Reject shrinking below stored materials; never silently discard stock.
    pub fn set_capacity(&mut self, capacity: u32) -> Result<(), CapacityError> {
        let stored = self.total_amount();
        if stored > u64::from(capacity) {
            return Err(CapacityError {
                requested: capacity,
                stored,
            });
        }
        self.max_capacity = capacity;
        Ok(())
    }

    pub fn total_amount(&self) -> u64 {
        self.resources
            .iter()
            .filter(|&(&res, _)| res != ResourceType::Labour)
            .map(|(_, &amount)| u64::from(amount))
            .sum()
    }
    pub fn add(&mut self, res_type: ResourceType, amount: u32) -> InventoryAddResult {
        let counter_space = u32::MAX - self.resources.get(&res_type).copied().unwrap_or(0);
        let available_space = if res_type == ResourceType::Labour {
            counter_space
        } else {
            u64::from(self.max_capacity)
                .saturating_sub(self.total_amount())
                .min(u64::from(counter_space)) as u32
        };
        let accepted = amount.min(available_space);
        if accepted > 0 {
            *self.resources.entry(res_type).or_insert(0) += accepted;
        }
        InventoryAddResult {
            accepted,
            rejected: amount - accepted,
        }
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
    fn additions_report_both_accepted_and_rejected_units() {
        let mut inventory = ResourceInventory::new(5);
        assert_eq!(
            inventory.add(ResourceType::Wood, 8),
            InventoryAddResult {
                accepted: 5,
                rejected: 3
            }
        );
        assert_eq!(
            inventory.add(ResourceType::Grain, 2),
            InventoryAddResult {
                accepted: 0,
                rejected: 2
            }
        );
        assert_eq!(inventory.total_amount(), 5);
    }

    #[test]
    fn shrinking_capacity_below_stock_is_rejected_without_changing_data() {
        let mut inventory = ResourceInventory::new(10);
        inventory.add(ResourceType::Wood, 6);
        assert_eq!(
            inventory.set_capacity(5),
            Err(CapacityError {
                requested: 5,
                stored: 6
            })
        );
        assert_eq!(inventory.capacity(), 10);
        assert_eq!(inventory.resources[&ResourceType::Wood], 6);
        inventory.set_capacity(6).unwrap();
        assert_eq!(inventory.add(ResourceType::Stone, 1).rejected, 1);
        inventory.set_capacity(7).unwrap();
        assert_eq!(inventory.add(ResourceType::Stone, 1).accepted, 1);
    }

    #[test]
    fn labour_only_obeys_its_counter_limit() {
        let mut inventory = ResourceInventory::new(0);
        assert_eq!(
            inventory.add(ResourceType::Labour, u32::MAX).accepted,
            u32::MAX
        );
        assert_eq!(
            inventory.add(ResourceType::Labour, 1),
            InventoryAddResult {
                accepted: 0,
                rejected: 1
            }
        );
        assert_eq!(inventory.total_amount(), 0);
        inventory.set_capacity(0).unwrap();
    }

    #[test]
    fn invalid_public_stock_above_u32_is_summed_without_overflow() {
        let mut inventory = ResourceInventory::new(u32::MAX);
        inventory.resources.insert(ResourceType::Wood, u32::MAX);
        inventory.resources.insert(ResourceType::Stone, u32::MAX);
        assert_eq!(inventory.total_amount(), 2 * u64::from(u32::MAX));
        assert_eq!(inventory.add(ResourceType::Grain, 1).rejected, 1);
    }

    #[test]
    fn labour_can_be_added_to_a_full_warehouse() {
        let mut inventory = ResourceInventory::new(1);
        inventory.add(ResourceType::Wood, 1);
        inventory.add(ResourceType::Labour, 10);
        assert_eq!(inventory.resources.get(&ResourceType::Labour), Some(&10));
        assert_eq!(inventory.total_amount(), 1);
    }

    #[test]
    fn an_overfull_public_inventory_rejects_additions_without_underflow() {
        let mut inventory = ResourceInventory::new(1);
        inventory.resources.insert(ResourceType::Wood, 2);
        inventory.add(ResourceType::Grain, 1);
        assert_eq!(inventory.total_amount(), 2);
        assert!(!inventory.resources.contains_key(&ResourceType::Grain));
    }

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
