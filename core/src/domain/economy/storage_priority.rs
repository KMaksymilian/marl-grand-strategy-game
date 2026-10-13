use crate::domain::economy::resource::ResourceType;
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoragePriorityError {
    LabourIsNotMaterial,
}

/// Settlement-local storage preference. Higher values are admitted first.
#[derive(Debug, Clone, Default)]
pub struct StoragePriorities {
    levels: HashMap<ResourceType, u32>,
}

impl StoragePriorities {
    pub fn set(
        &mut self,
        resource: ResourceType,
        priority: u32,
    ) -> Result<(), StoragePriorityError> {
        if resource == ResourceType::Labour {
            return Err(StoragePriorityError::LabourIsNotMaterial);
        }
        if priority == 0 {
            self.levels.remove(&resource);
        } else {
            self.levels.insert(resource, priority);
        }
        Ok(())
    }

    pub fn get(&self, resource: ResourceType) -> u32 {
        self.levels.get(&resource).copied().unwrap_or(0)
    }

    pub(crate) fn compare(&self, a: ResourceType, b: ResourceType) -> Ordering {
        self.get(b).cmp(&self.get(a)).then_with(|| a.cmp(&b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_resets_a_priority_and_maximum_values_compare_without_overflow() {
        let mut priorities = StoragePriorities::default();
        assert_eq!(priorities.get(ResourceType::Grain), 0);
        priorities.set(ResourceType::Grain, u32::MAX).unwrap();
        assert_eq!(
            priorities.compare(ResourceType::Grain, ResourceType::Wood),
            Ordering::Less
        );
        priorities.set(ResourceType::Grain, 0).unwrap();
        assert_eq!(priorities.get(ResourceType::Grain), 0);
        assert_eq!(
            priorities.compare(ResourceType::Grain, ResourceType::Wood),
            Ordering::Greater
        );
    }

    #[test]
    fn labour_cannot_receive_a_material_storage_priority() {
        let mut priorities = StoragePriorities::default();
        priorities.set(ResourceType::Grain, 5).unwrap();
        assert_eq!(
            priorities.set(ResourceType::Labour, 10),
            Err(StoragePriorityError::LabourIsNotMaterial)
        );
        assert_eq!(priorities.get(ResourceType::Labour), 0);
        assert_eq!(priorities.get(ResourceType::Grain), 5);
    }
}
