use crate::domain::economy::{ResourceCatalog, ResourceType};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageOrderError {
    LabourIsNotMaterial,
    DuplicateMaterial(ResourceType),
    MissingMaterial(ResourceType),
    UnknownMaterial(ResourceType),
}

/// Persistent preference list, ordered from first admitted to last.
#[derive(Debug, Clone)]
pub struct StorageOrder {
    catalog_materials: Vec<ResourceType>,
    materials: Vec<ResourceType>,
}
impl Default for StorageOrder {
    fn default() -> Self {
        Self::for_catalog(ResourceCatalog::standard())
    }
}
impl StorageOrder {
    pub fn for_catalog(catalog: &ResourceCatalog) -> Self {
        Self {
            catalog_materials: catalog.materials().to_vec(),
            materials: catalog.materials().to_vec(),
        }
    }

    pub fn set(&mut self, materials: impl AsRef<[ResourceType]>) -> Result<(), StorageOrderError> {
        let materials = materials.as_ref();
        self.validate_order(materials)?;
        self.materials = materials.to_vec();
        Ok(())
    }

    /// Validate an order against the bundled catalogue.
    pub fn validate(materials: &[ResourceType]) -> Result<(), StorageOrderError> {
        Self::validate_against(materials, ResourceCatalog::standard().materials())
    }

    /// Validate an order against this warehouse's catalogue without applying it.
    pub fn validate_order(&self, materials: &[ResourceType]) -> Result<(), StorageOrderError> {
        Self::validate_against(materials, &self.catalog_materials)
    }

    fn validate_against(
        materials: &[ResourceType],
        catalog_materials: &[ResourceType],
    ) -> Result<(), StorageOrderError> {
        let mut seen = HashSet::with_capacity(materials.len());
        for resource in materials {
            if resource == &ResourceType::Labour {
                return Err(StorageOrderError::LabourIsNotMaterial);
            }
            if !seen.insert(resource) {
                return Err(StorageOrderError::DuplicateMaterial(resource.clone()));
            }
        }
        let available: HashSet<_> = catalog_materials.iter().collect();
        for resource in materials {
            if !available.contains(resource) {
                return Err(StorageOrderError::UnknownMaterial(resource.clone()));
            }
        }
        for resource in catalog_materials {
            if !seen.contains(resource) {
                return Err(StorageOrderError::MissingMaterial(resource.clone()));
            }
        }
        Ok(())
    }
    pub fn get(&self) -> &[ResourceType] {
        &self.materials
    }
    pub(crate) fn compare(&self, a: &ResourceType, b: &ResourceType) -> Ordering {
        let rank = |resource: &ResourceType| {
            self.materials
                .iter()
                .position(|item| item == resource)
                .unwrap_or(self.materials.len())
        };
        rank(a).cmp(&rank(b))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn material(id: &str) -> ResourceType {
        ResourceCatalog::standard().resolve(id).unwrap()
    }

    #[test]
    fn validation_checks_the_same_rules_without_changing_an_order() {
        let mut complete = ResourceCatalog::standard().materials().to_vec();
        complete[..3].reverse();
        assert_eq!(StorageOrder::validate(&complete), Ok(()));
        assert_eq!(
            StorageOrder::validate(&[material("wood"), material("wood"), material("grain")]),
            Err(StorageOrderError::DuplicateMaterial(material("wood")))
        );
        assert_eq!(
            StorageOrder::validate(&[material("wood"), material("stone")]),
            Err(StorageOrderError::MissingMaterial(material("grain")))
        );
        assert_eq!(
            StorageOrder::validate(&[
                ResourceType::Labour,
                material("wood"),
                material("stone"),
                material("grain"),
            ]),
            Err(StorageOrderError::LabourIsNotMaterial)
        );
    }

    #[test]
    fn invalid_orders_leave_previous_order_unchanged() {
        let mut order = StorageOrder::default();
        let mut preferred = order.get().to_vec();
        preferred[..3].rotate_right(1);
        order.set(&preferred).unwrap();
        assert_eq!(
            order.set([material("wood"), material("wood"), material("grain")]),
            Err(StorageOrderError::DuplicateMaterial(material("wood")))
        );
        assert_eq!(
            order.set([material("wood"), material("stone"), ResourceType::Labour]),
            Err(StorageOrderError::LabourIsNotMaterial)
        );
        assert_eq!(order.get(), preferred);
        assert_eq!(
            order.compare(&material("grain"), &material("wood")),
            Ordering::Less
        );
    }
}
