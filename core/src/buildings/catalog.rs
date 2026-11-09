//! Registry of shared, validated building definitions.
use super::definition::BuildingDefinition;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub struct BuildingCatalog {
    registry: HashMap<String, Arc<BuildingDefinition>>,
}

impl BuildingCatalog {
    pub fn register(&mut self, definition: BuildingDefinition) {
        self.registry
            .insert(definition.id().to_owned(), Arc::new(definition));
    }

    pub fn get_blueprint(&self, id: &str) -> Option<Arc<BuildingDefinition>> {
        self.registry.get(id).cloned()
    }

    /// Registered building type IDs in ascending order.
    pub fn ids(&self) -> Vec<&str> {
        let mut ids: Vec<_> = self.registry.keys().map(String::as_str).collect();
        ids.sort_unstable();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::super::definition::BuildingBehavior;
    use super::*;
    use crate::buildings::effects::StatType;

    fn create_dummy_definition(id: &str, workers: u32) -> BuildingDefinition {
        BuildingDefinition::new(
            id.to_string(),
            "Test Building".to_string(),
            1,
            workers,
            vec![BuildingBehavior::StatModifier {
                stat: StatType::HousingCapacity,
                base_amount: 10,
                per_worker_amount: 2,
            }],
            Default::default(),
        )
        .unwrap()
    }

    #[test]
    fn test_catalog_initialization() {
        let catalog = BuildingCatalog::default();
        assert!(
            catalog.registry.is_empty(),
            "Factory should initialize empty registry"
        );
    }

    #[test]
    fn test_register_and_get_blueprint() {
        let mut catalog = BuildingCatalog::default();
        let def = create_dummy_definition("house_t1", 5);

        catalog.register(def);

        let blueprint = catalog.get_blueprint("house_t1");
        assert!(blueprint.is_some(), "Blueprint should be initialized");

        let blueprint = blueprint.unwrap();
        assert_eq!(blueprint.id(), "house_t1");
        assert_eq!(blueprint.name(), "Test Building");
        assert_eq!(blueprint.max_workers(), 5);
    }

    #[test]
    fn test_get_nonexistent_blueprint() {
        let catalog = BuildingCatalog::default();

        let blueprint = catalog.get_blueprint("magic_tower");
        assert!(
            blueprint.is_none(),
            "For non-existent blueprint catalog should return None"
        );
    }

    #[test]
    fn test_flyweight_pattern_arc_cloning() {
        let mut catalog = BuildingCatalog::default();
        let def = create_dummy_definition("lumber_mill", 5);
        catalog.register(def);

        let bp1 = catalog.get_blueprint("lumber_mill").unwrap();
        let bp2 = catalog.get_blueprint("lumber_mill").unwrap();

        assert!(
            Arc::ptr_eq(&bp1, &bp2),
            "Both blueprint catalog should have been flyweight patterns"
        );
    }

    #[test]
    fn test_register_overwrites_existing_id() {
        let mut catalog = BuildingCatalog::default();

        let def1 = create_dummy_definition("forge", 3);
        catalog.register(def1);

        let def2 = create_dummy_definition("forge", 10);
        catalog.register(def2);

        let blueprint = catalog.get_blueprint("forge").unwrap();

        assert_eq!(blueprint.max_workers(), 10);
    }
}
