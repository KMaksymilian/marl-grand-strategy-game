use crate::domain::economy::resource::ResourceType;
use crate::domain::settlement::stat::StatType;
use std::collections::HashMap;
use std::sync::Arc;
use crate::domain::settlement::projects::ProjectCost;

#[derive(Debug, Clone)]
pub enum BuildingBehavior {
    Production {
        inputs: Vec<(ResourceType, u32)>,
        outputs: Vec<(ResourceType, u32)>,
    },
    StatModifier {
        stat: StatType,
        base_amount: u32,
        per_worker_amount: u32,
    },
}

#[derive(Debug)]
pub struct BuildingDefinition {
    pub id: String,
    pub name: String,
    pub tier: u8,
    pub max_workers: u32,
    pub behaviors: Vec<BuildingBehavior>,
    pub construction_cost: ProjectCost,
}

#[derive(Default)]
pub struct BuildingFactory {
    registry: HashMap<String, Arc<BuildingDefinition>>,
}

impl BuildingFactory {
    pub fn register(&mut self, definition: BuildingDefinition) {
        self.registry
            .insert(definition.id.clone(), Arc::new(definition));
    }

    pub fn get_blueprint(&self, id: &str) -> Option<Arc<BuildingDefinition>> {
        self.registry.get(id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_factory_initialization() {
        let factory = BuildingFactory::default();
        assert!(
            factory.registry.is_empty(),
            "Factory should initialize empty registry"
        );
    }

    #[test]
    fn test_register_and_get_blueprint() {
        let mut factory = BuildingFactory::default();
        let def = create_dummy_definition("house_t1");

        factory.register(def);

        let blueprint = factory.get_blueprint("house_t1");
        assert!(blueprint.is_some(), "Blueprint should be initialized");

        let blueprint = blueprint.unwrap();
        assert_eq!(blueprint.id, "house_t1");
        assert_eq!(blueprint.name, "Test Building");
        assert_eq!(blueprint.max_workers, 5);
    }

    #[test]
    fn test_get_nonexistent_blueprint() {
        let factory = BuildingFactory::default();

        let blueprint = factory.get_blueprint("magic_tower");
        assert!(
            blueprint.is_none(),
            "For non-existent blueprint factory should return None"
        );
    }

    #[test]
    fn test_flyweight_pattern_arc_cloning() {
        let mut factory = BuildingFactory::default();
        let def = create_dummy_definition("lumber_mill");
        factory.register(def);

        let bp1 = factory.get_blueprint("lumber_mill").unwrap();
        let bp2 = factory.get_blueprint("lumber_mill").unwrap();

        assert!(
            Arc::ptr_eq(&bp1, &bp2),
            "Both blueprint factory should have been flyweight patterns"
        );
    }

    #[test]
    fn test_register_overwrites_existing_id() {
        let mut factory = BuildingFactory::default();

        let mut def1 = create_dummy_definition("forge");
        def1.max_workers = 3;
        factory.register(def1);

        let mut def2 = create_dummy_definition("forge");
        def2.max_workers = 10;
        factory.register(def2);

        let blueprint = factory.get_blueprint("forge").unwrap();

        assert_eq!(blueprint.max_workers, 10);
    }
}
