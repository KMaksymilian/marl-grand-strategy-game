use crate::domain::economy::ResourceType;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageOrderError {
    LabourIsNotMaterial,
    DuplicateMaterial(ResourceType),
    MissingMaterial(ResourceType),
}

/// Persistent preference list, ordered from first admitted to last.
#[derive(Debug, Clone)]
pub struct StorageOrder {
    materials: Vec<ResourceType>,
}
impl Default for StorageOrder {
    fn default() -> Self {
        Self {
            materials: ResourceType::MATERIALS.to_vec(),
        }
    }
}
impl StorageOrder {
    pub fn set(&mut self, materials: impl AsRef<[ResourceType]>) -> Result<(), StorageOrderError> {
        let materials = materials.as_ref();
        Self::validate(materials)?;
        self.materials = materials.to_vec();
        Ok(())
    }

    /// The rule used by `set`, without changing any order.
    pub fn validate(materials: &[ResourceType]) -> Result<(), StorageOrderError> {
        for (index, resource) in materials.iter().enumerate() {
            if *resource == ResourceType::Labour {
                return Err(StorageOrderError::LabourIsNotMaterial);
            }
            if materials[..index].contains(resource) {
                return Err(StorageOrderError::DuplicateMaterial(*resource));
            }
        }
        for resource in ResourceType::MATERIALS {
            if !materials.contains(resource) {
                return Err(StorageOrderError::MissingMaterial(*resource));
            }
        }
        Ok(())
    }
    pub fn get(&self) -> &[ResourceType] {
        &self.materials
    }
    pub(crate) fn compare(&self, a: ResourceType, b: ResourceType) -> Ordering {
        let rank = |resource| {
            self.materials
                .iter()
                .position(|item| *item == resource)
                .unwrap_or(self.materials.len())
        };
        rank(a).cmp(&rank(b))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ResourceType::*;
    #[test]
    fn validation_checks_the_same_rules_without_changing_an_order() {
        assert_eq!(StorageOrder::validate(&[Grain, Stone, Wood]), Ok(()));
        assert_eq!(
            StorageOrder::validate(&[Wood, Wood, Grain]),
            Err(StorageOrderError::DuplicateMaterial(Wood))
        );
        assert_eq!(
            StorageOrder::validate(&[Wood, Stone]),
            Err(StorageOrderError::MissingMaterial(Grain))
        );
        assert_eq!(
            StorageOrder::validate(&[Labour, Wood, Stone, Grain]),
            Err(StorageOrderError::LabourIsNotMaterial)
        );
    }

    #[test]
    fn invalid_orders_leave_previous_order_unchanged() {
        let mut order = StorageOrder::default();
        order.set([Grain, Wood, Stone]).unwrap();
        assert_eq!(
            order.set([Wood, Wood, Grain]),
            Err(StorageOrderError::DuplicateMaterial(Wood))
        );
        assert_eq!(
            order.set([Wood, Stone, Labour]),
            Err(StorageOrderError::LabourIsNotMaterial)
        );
        assert_eq!(order.get(), &[Grain, Wood, Stone]);
        assert_eq!(order.compare(Grain, Wood), Ordering::Less);
    }
}
