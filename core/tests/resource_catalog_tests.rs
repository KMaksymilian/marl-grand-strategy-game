use simulation_core::buildings::Building;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::economy::{
    FoodClass, ResourceCatalog, ResourceCatalogError, ResourceCategory, ResourceType,
};
use simulation_core::ids::{BuildingId, ProjectId, SettlementId};
use simulation_core::settlement::projects::ProjectCost;
use simulation_core::settlement::turn_report::{InventoryLoss, InventoryLossStage};
use simulation_core::settlement::{NeedRegistry, NeedTier, Settlement};
use simulation_core::simulation::Simulation;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::terrain::area::Area;
use simulation_core::world::{Map, Position, Province, World};
use std::collections::HashMap;
use std::sync::Arc;

const RESOURCE_TOML: &str = include_str!("../../resources.toml");
const SETTLEMENT: SettlementId = SettlementId(1);

#[test]
fn startup_catalog_loads_from_disk() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources.toml");
    let catalog = ResourceCatalog::load(path).unwrap();

    assert_eq!(catalog.materials(), ResourceCatalog::standard().materials());
    assert_eq!(catalog.resolve("charcoal"), Some(ResourceType::Coal));
}

#[test]
fn standard_catalog_contains_the_approved_materials_and_food_classes() {
    use FoodClass::{Processed, Raw};
    use ResourceType::*;

    let catalog = ResourceCatalog::standard();
    type MaterialGroup<'a> = (u8, Option<FoodClass>, &'a [(&'a str, ResourceType)]);
    let groups: &[MaterialGroup<'_>] = &[
        (
            1,
            Some(Raw),
            &[
                ("grain", Grain),
                ("cattle", Cattle),
                ("fish", Fish),
                ("dates", Dates),
            ],
        ),
        (
            1,
            None,
            &[
                ("wood", Wood),
                ("stone", Stone),
                ("clay", Clay),
                ("limestone", Limestone),
                ("coal", Coal),
                ("iron_ore", IronOre),
                ("copper_ore", CopperOre),
                ("tin_ore", TinOre),
            ],
        ),
        (
            2,
            Some(Processed),
            &[
                ("bread", Bread),
                ("meat", Meat),
                ("smoked_fish", SmokedFish),
                ("date_syrup", DateSyrup),
            ],
        ),
        (
            2,
            None,
            &[
                ("planks", Planks),
                ("stone_bricks", StoneBricks),
                ("bricks", Bricks),
                ("steel", Steel),
                ("bronze", Bronze),
                ("beer", Beer),
            ],
        ),
        (
            3,
            None,
            &[
                ("fortification_components", FortificationComponents),
                ("furniture", Furniture),
                ("tools", Tools),
                ("monument_components", MonumentComponents),
            ],
        ),
    ];

    for (tier, food_class, resources) in groups {
        for (name, resource) in *resources {
            assert_eq!(catalog.resolve(name), Some(*resource), "{name}");
            assert!(catalog.materials().contains(resource), "{name}");
            let definition = catalog.get(*resource).unwrap();
            assert_eq!(definition.tier(), *tier, "{name}");
            assert_eq!(definition.food_class(), *food_class, "{name}");
        }
    }
    assert_eq!(
        catalog.materials().len(),
        groups
            .iter()
            .map(|(_, _, resources)| resources.len())
            .sum::<usize>()
    );
    assert_eq!(catalog.resolve("charcoal"), Some(Coal));
    assert_eq!(catalog.resolve("mined_coal"), Some(Coal));
    assert!(catalog.get(Labour).is_none());
    assert_eq!(catalog.resolve("unknown_material"), None);
    assert_eq!(
        catalog.get(Beer).unwrap().category(),
        ResourceCategory::Beverage
    );
    assert_eq!(
        Settlement::new(SETTLEMENT, 10, 1, Arc::default()).storage_order(),
        catalog.materials()
    );
}

#[test]
fn loaded_catalog_supplies_metadata_and_settlement_storage_order() {
    use ResourceType::{Grain, Stone, Wood};

    let mut source: toml::Table = toml::from_str(RESOURCE_TOML).unwrap();
    let resources = source.get_mut("resources").unwrap().as_array_mut().unwrap();
    resources.swap(0, 1);
    let grain = resources
        .iter_mut()
        .find(|entry| entry["id"].as_str() == Some("grain"))
        .unwrap()
        .as_table_mut()
        .unwrap();
    grain.insert("category".into(), "processed_food".into());
    grain.insert("tier".into(), 2.into());
    grain.insert("aliases".into(), toml::Value::Array(vec!["cereal".into()]));

    let catalog = ResourceCatalog::from_toml(&toml::to_string(&source).unwrap()).unwrap();
    assert_eq!(catalog.resolve("cereal"), Some(Grain));
    let definition = catalog.get(Grain).unwrap();
    assert_eq!(definition.tier(), 2);
    assert_eq!(definition.food_class(), Some(FoodClass::Processed));
    assert_eq!(ResourceCatalog::standard().resolve("cereal"), None);

    let settlement = Settlement::with_resource_catalog(SETTLEMENT, 10, 1, Arc::default(), &catalog);
    assert_eq!(&settlement.storage_order()[..2], &[Stone, Wood]);
    assert_eq!(
        &Settlement::new(SETTLEMENT, 10, 1, Arc::default()).storage_order()[..2],
        &[Wood, Stone]
    );
}

#[test]
fn invalid_catalogs_are_rejected() {
    fn check_invalid(change: impl FnOnce(&mut Vec<toml::Value>)) {
        let mut source: toml::Table = toml::from_str(RESOURCE_TOML).unwrap();
        change(source.get_mut("resources").unwrap().as_array_mut().unwrap());
        let source = toml::to_string(&source).unwrap();
        assert!(ResourceCatalog::from_toml(&source).is_err(), "{source}");
    }

    check_invalid(|resources| {
        resources.remove(0);
    });
    check_invalid(|resources| resources.push(resources[0].clone()));
    check_invalid(|resources| resources[0]["id"] = "labour".into());
    check_invalid(|resources| resources[0]["tier"] = 0.into());
    check_invalid(|resources| {
        resources[0]
            .as_table_mut()
            .unwrap()
            .insert("aliases".into(), toml::Value::Array(vec!["stone".into()]));
    });
}

#[test]
fn loaded_priority_admits_new_material_first_when_production_exceeds_capacity() {
    use ResourceType::{Beer, Wood};

    let mut source: toml::Table = toml::from_str(RESOURCE_TOML).unwrap();
    let resources = source.get_mut("resources").unwrap().as_array_mut().unwrap();
    let beer_index = resources
        .iter()
        .position(|entry| entry["id"].as_str() == Some("beer"))
        .unwrap();
    let beer = resources.remove(beer_index);
    resources.insert(0, beer);
    let catalog = ResourceCatalog::from_toml(&toml::to_string(&source).unwrap()).unwrap();

    let position = Position { x: 0, y: 0 };
    let map = Map::new(1, 1, vec![Area::new(0.5, 0.5)], vec![0]);
    let mut world = World::new(map, vec![Province::new(0)]);
    let definition = Arc::new(
        BuildingDefinition::new(
            "test_workshop",
            "Test Workshop",
            2,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(Wood, 2), (Beer, 2)],
            }],
            ProjectCost::default(),
        )
        .unwrap(),
    );
    let mut building = Building::new(BuildingId(1), position, definition);
    building.set_target_load(1.0).unwrap();
    let settlement = Settlement::with_resource_catalog(SETTLEMENT, 3, 1, Arc::default(), &catalog);
    world.add_settlement_at(0, position, settlement).unwrap();
    world
        .add_building(SETTLEMENT, building, &mut PathFinder::new(1, 1))
        .unwrap();

    let mut simulation = Simulation::new(world).unwrap();
    let reports = simulation.step().unwrap();
    let report = &reports.settlements[&SETTLEMENT];
    let settlement = simulation.world().settlement(SETTLEMENT).unwrap();

    assert_eq!(settlement.inventory().amount(Beer), 2);
    assert_eq!(settlement.inventory().amount(Wood), 1);
    assert_eq!(
        report.inventory.losses,
        vec![InventoryLoss {
            resource: Wood,
            amount: 1,
            stage: InventoryLossStage::ProductionBuffer,
        }]
    );
    assert!(report.inventory.is_balanced());
}

#[test]
fn coal_source_aliases_are_required_and_cannot_be_reassigned() {
    let missing_alias = RESOURCE_TOML.replace(", \"mined_coal\"", "");
    assert!(matches!(
        ResourceCatalog::from_toml(&missing_alias),
        Err(ResourceCatalogError::InvalidCoalAlias("mined_coal"))
    ));

    let reassigned_alias =
        missing_alias.replace("id = \"wood\"", "id = \"wood\"\naliases = [\"mined_coal\"]");
    assert!(matches!(
        ResourceCatalog::from_toml(&reassigned_alias),
        Err(ResourceCatalogError::InvalidCoalAlias("mined_coal"))
    ));
}

#[test]
fn stock_recipe_project_and_population_share_unchanged_whole_units() {
    use ResourceType::{Fish, MonumentComponents, SmokedFish};

    let position = Position { x: 0, y: 0 };
    let map = Map::new(2, 2, vec![Area::new(0.5, 0.5); 4], vec![0; 4]);
    let mut world = World::new(map, vec![Province::new(0)]);
    let needs = NeedRegistry::new(vec![NeedTier {
        min_population: 2,
        needs_per_capita: HashMap::from([(SmokedFish, 1)]),
    }]);
    let mut settlement = Settlement::new(SETTLEMENT, 20, 2, Arc::new(needs));
    assert_eq!(settlement.deposit(Fish, 3).accepted, 3);
    assert_eq!(settlement.deposit(SmokedFish, 2).accepted, 2);
    assert_eq!(settlement.deposit(MonumentComponents, 4).accepted, 4);

    let smokehouse = Arc::new(
        BuildingDefinition::new(
            "smokehouse",
            "Smokehouse",
            2,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![(Fish, 3)],
                outputs: vec![(SmokedFish, 3)],
            }],
            ProjectCost::default(),
        )
        .unwrap(),
    );
    let mut building = Building::new(BuildingId(1), position, smokehouse);
    building.set_target_load(1.0).unwrap();
    world.add_settlement_at(0, position, settlement).unwrap();
    world
        .add_building(SETTLEMENT, building, &mut PathFinder::new(2, 2))
        .unwrap();

    let cost = ProjectCost {
        needed_resources: HashMap::from([(MonumentComponents, 3)]),
        turn_cost: 1,
    };
    let monument = Arc::new(
        BuildingDefinition::new("monument", "Monument", 3, 0, vec![], cost.clone()).unwrap(),
    );
    world
        .queue_construction(
            SETTLEMENT,
            ProjectId(1),
            Position { x: 1, y: 0 },
            monument,
            cost,
        )
        .unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    let reports = simulation.step().unwrap();
    let report = &reports.settlements[&SETTLEMENT];
    let settlement = simulation.world().settlement(SETTLEMENT).unwrap();

    assert_eq!(settlement.population(), 2);
    assert_eq!(report.inventory.opening_stock[&Fish], 3);
    assert_eq!(report.inventory.production_consumed[&Fish], 3);
    assert_eq!(report.inventory.produced[&SmokedFish], 3);
    assert_eq!(report.inventory.population_consumed[&SmokedFish], 2);
    assert!(report.consumption.missing_resources.is_empty());
    assert_eq!(report.inventory.project_consumed[&MonumentComponents], 3);
    assert!(report.projects[0].result.is_ok());
    assert_eq!(settlement.inventory().amount(SmokedFish), 3);
    assert_eq!(settlement.inventory().amount(MonumentComponents), 1);
    assert!(report.inventory.is_balanced());
}
