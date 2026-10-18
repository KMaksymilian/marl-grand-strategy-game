//! Registry of shared, validated building definitions.
use super::definition::{BuildingDefinition, DefinitionError};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub struct BuildingCatalog {
    registry: HashMap<String, Arc<BuildingDefinition>>,
}

impl BuildingCatalog {
    pub fn register(&mut self, definition: BuildingDefinition) -> Result<(), DefinitionError> {
        definition.validate()?;
        self.registry
            .insert(definition.id.clone(), Arc::new(definition));
        Ok(())
    }

    pub fn get_blueprint(&self, id: &str) -> Option<Arc<BuildingDefinition>> {
        self.registry.get(id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::super::definition::BuildingBehavior;
    use super::*;
    use crate::settlement::stat::StatType;

    fn create_dummy_definition(id: &str) -> BuildingDefinition {
        BuildingDefinition {
            id: id.to_string(),
            name: "Test Building".to_string(),
            tier: 1,
            max_workers: 5,
            behaviors: vec![BuildingBehavior::StatModifier {
                stat: StatType::HousingCapacity,
                base_amount: 10,
                per_worker_amount: 2,
            }],
            construction_cost: Default::default(),
        }
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
        let def = create_dummy_definition("house_t1");

        catalog.register(def).unwrap();

        let blueprint = catalog.get_blueprint("house_t1");
        assert!(blueprint.is_some(), "Blueprint should be initialized");

        let blueprint = blueprint.unwrap();
        assert_eq!(blueprint.id, "house_t1");
        assert_eq!(blueprint.name, "Test Building");
        assert_eq!(blueprint.max_workers, 5);
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
        let def = create_dummy_definition("lumber_mill");
        catalog.register(def).unwrap();

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

        let mut def1 = create_dummy_definition("forge");
        def1.max_workers = 3;
        catalog.register(def1).unwrap();

        let mut def2 = create_dummy_definition("forge");
        def2.max_workers = 10;
        catalog.register(def2).unwrap();

        let blueprint = catalog.get_blueprint("forge").unwrap();

        assert_eq!(blueprint.max_workers, 10);
    }
}
