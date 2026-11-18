mod common;
use common::material;
use simulation_core::buildings::Building;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::economy::storage_order::StorageOrderError;
use simulation_core::economy::{
    FoodClass, ResourceCatalog, ResourceCatalogError, ResourceCategory, ResourceType,
};
use simulation_core::ids::{BuildingId, ProjectId, SettlementId};
use simulation_core::settlement::decision::{OfferSide, OfferSpec};
use simulation_core::settlement::projects::ProjectCost;
use simulation_core::settlement::turn_report::{InventoryLoss, InventoryLossStage};
use simulation_core::settlement::{NeedRegistry, NeedTier, Settlement};
use simulation_core::simulation::{
    ActionError, ActionOutcome, SettlementAction, SettlementDecision, Simulation,
};
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
    assert_eq!(catalog.resolve("charcoal"), Some(material("coal")));
}

#[test]
fn standard_catalog_contains_the_approved_materials_and_food_classes() {
    use FoodClass::{Processed, Raw};

    let catalog = ResourceCatalog::standard();
    type MaterialGroup<'a> = (u8, Option<FoodClass>, &'a [(&'a str, ResourceType)]);
    let groups: &[MaterialGroup<'_>] = &[
        (
            1,
            Some(Raw),
            &[
                ("grain", material("grain")),
                ("cattle", material("cattle")),
                ("fish", material("fish")),
                ("dates", material("dates")),
            ],
        ),
        (
            1,
            None,
            &[
                ("wood", material("wood")),
                ("stone", material("stone")),
                ("clay", material("clay")),
                ("limestone", material("limestone")),
                ("coal", material("coal")),
                ("iron_ore", material("iron_ore")),
                ("copper_ore", material("copper_ore")),
                ("tin_ore", material("tin_ore")),
            ],
        ),
        (
            2,
            Some(Processed),
            &[
                ("bread", material("bread")),
                ("meat", material("meat")),
                ("smoked_fish", material("smoked_fish")),
                ("date_syrup", material("date_syrup")),
            ],
        ),
        (
            2,
            None,
            &[
                ("planks", material("planks")),
                ("stone_bricks", material("stone_bricks")),
                ("bricks", material("bricks")),
                ("steel", material("steel")),
                ("bronze", material("bronze")),
                ("beer", material("beer")),
            ],
        ),
        (
            3,
            None,
            &[
                (
                    "fortification_components",
                    material("fortification_components"),
                ),
                ("furniture", material("furniture")),
                ("tools", material("tools")),
                ("monument_components", material("monument_components")),
            ],
        ),
    ];

    for (tier, food_class, resources) in groups {
        for (name, resource) in *resources {
            assert_eq!(catalog.resolve(name).as_ref(), Some(resource), "{name}");
            assert!(catalog.materials().contains(resource), "{name}");
            let definition = catalog.get(resource.clone()).unwrap();
            assert_eq!(definition.tier(), *tier, "{name}");
            assert_eq!(definition.food_class(), *food_class, "{name}");
        }
    }
    assert_eq!(catalog.resolve("charcoal"), Some(material("coal")));
    assert_eq!(catalog.resolve("mined_coal"), Some(material("coal")));
    assert!(catalog.get(ResourceType::Labour).is_none());
    assert_eq!(catalog.resolve("unknown_material"), None);
    assert_eq!(
        catalog.get(material("beer")).unwrap().category(),
        ResourceCategory::Beverage
    );
    assert_eq!(
        Settlement::new(SETTLEMENT, 10, 1, Arc::default()).storage_order(),
        catalog.materials()
    );
}

#[test]
fn loaded_catalog_supplies_metadata_and_settlement_storage_order() {
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
    assert_eq!(catalog.resolve("cereal"), Some(material("grain")));
    let definition = catalog.get(material("grain")).unwrap();
    assert_eq!(definition.tier(), 2);
    assert_eq!(definition.food_class(), Some(FoodClass::Processed));
    assert_eq!(ResourceCatalog::standard().resolve("cereal"), None);

    let settlement = Settlement::with_resource_catalog(SETTLEMENT, 10, 1, Arc::default(), &catalog);
    assert_eq!(
        &settlement.storage_order()[..2],
        &[material("stone"), material("wood")]
    );
    assert_eq!(
        &Settlement::new(SETTLEMENT, 10, 1, Arc::default()).storage_order()[..2],
        &[material("wood"), material("stone")]
    );
}

#[test]
fn custom_catalog_does_not_require_bundled_materials() {
    let catalog = ResourceCatalog::from_toml(
        r#"
        [[resources]]
        id = "salt"
        name = "Salt"
        tier = 1
        category = "crafted_good"
        "#,
    )
    .unwrap();
    let salt = catalog.resolve("salt").unwrap();
    let settlement = Settlement::with_resource_catalog(SETTLEMENT, 10, 1, Arc::default(), &catalog);

    assert_eq!(settlement.storage_order(), &[salt]);
    assert!(catalog.resolve("wood").is_none());
    assert!(catalog.resolve("charcoal").is_none());
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
        resources[0]["id"] = "bad material".into();
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
fn catalog_material_uses_the_same_production_storage_and_decision_paths() {
    let new_material = r#"
        [[resources]]
        id = "scenario_test_salt"
        name = "Scenario Test Salt"
        tier = 1
        category = "crafted_good"
    "#;
    let catalog = ResourceCatalog::from_toml(&format!("{RESOURCE_TOML}\n{new_material}")).unwrap();
    let salt = catalog.resolve("scenario_test_salt").unwrap();
    for (material, wire_name) in [
        (salt.clone(), "\"ScenarioTestSalt\""),
        (material("wood"), "\"Wood\""),
        (material("iron_ore"), "\"IronOre\""),
        (ResourceType::Labour, "\"Labour\""),
    ] {
        assert_eq!(serde_json::to_string(&material).unwrap(), wire_name);
        assert_eq!(
            serde_json::from_str::<ResourceType>(wire_name).unwrap(),
            material
        );
    }
    assert_eq!(
        serde_json::to_value(HashMap::from([(salt.clone(), 2)])).unwrap(),
        serde_json::json!({"ScenarioTestSalt": 2}),
    );
    let mut order = catalog.materials().to_vec();
    order.rotate_right(1);
    assert_eq!(order.first(), Some(&salt));
    let mut invalid_order = order.clone();
    let unknown = serde_json::from_str::<ResourceType>("\"UnknownMaterial\"").unwrap();
    invalid_order.push(unknown.clone());

    let position = Position { x: 0, y: 0 };
    let map = Map::new(1, 1, vec![Area::new(0.5, 0.5)], vec![0]);
    let mut world = World::with_resource_catalog(map, vec![Province::new(0)], Arc::new(catalog));
    let definition = Arc::new(
        BuildingDefinition::new(
            "test_workshop",
            "Test Workshop",
            2,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![],
                outputs: vec![(material("wood"), 2), (salt.clone(), 2)],
            }],
            ProjectCost::default(),
        )
        .unwrap(),
    );
    let mut building = Building::new(BuildingId(1), position, definition);
    building.set_target_load(1.0).unwrap();
    let settlement_id = world
        .initialize_settlement_at(0, position, 3, 1, Arc::default())
        .unwrap();
    world
        .add_building(settlement_id, building, &mut PathFinder::new(1, 1))
        .unwrap();

    let mut simulation = Simulation::new(world).unwrap();
    simulation
        .queue_decision(SettlementDecision::new(
            settlement_id,
            vec![
                SettlementAction::SetStorageOrder {
                    order: invalid_order,
                },
                SettlementAction::SetStorageOrder {
                    order: order.clone(),
                },
                SettlementAction::PublishMarketOffers {
                    offers: vec![OfferSpec {
                        resource: unknown.clone(),
                        side: OfferSide::Buy,
                        units: 1,
                        price_per_unit: 1,
                    }],
                },
            ],
        ))
        .unwrap();
    let reports = simulation.step().unwrap();
    let report = &reports.settlements[&settlement_id];
    let settlement = simulation.world().settlement(settlement_id).unwrap();

    let decisions = &reports.decisions[0].results;
    assert_eq!(
        decisions[0].outcome,
        ActionOutcome::Rejected {
            error: ActionError::InvalidStorageOrder {
                error: StorageOrderError::UnknownMaterial(unknown.clone()),
            },
        },
    );
    assert!(decisions[1].is_applied());
    assert_eq!(
        decisions[2].outcome,
        ActionOutcome::Rejected {
            error: ActionError::UnknownMaterial { resource: unknown },
        },
    );
    assert_eq!(settlement.storage_order(), order);
    assert_eq!(settlement.inventory().amount(salt), 2);
    assert_eq!(settlement.inventory().amount(material("wood")), 1);
    assert_eq!(
        report.inventory.losses,
        vec![InventoryLoss {
            resource: material("wood"),
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
    let position = Position { x: 0, y: 0 };
    let map = Map::new(2, 2, vec![Area::new(0.5, 0.5); 4], vec![0; 4]);
    let mut world = World::new(map, vec![Province::new(0)]);
    let needs = NeedRegistry::new(vec![NeedTier {
        min_population: 2,
        needs_per_capita: HashMap::from([(material("smoked_fish"), 1)]),
    }]);
    let mut settlement = Settlement::new(SETTLEMENT, 20, 2, Arc::new(needs));
    assert_eq!(settlement.deposit(material("fish"), 3).accepted, 3);
    assert_eq!(settlement.deposit(material("smoked_fish"), 2).accepted, 2);
    assert_eq!(
        settlement
            .deposit(material("monument_components"), 4)
            .accepted,
        4
    );

    let smokehouse = Arc::new(
        BuildingDefinition::new(
            "smokehouse",
            "Smokehouse",
            2,
            1,
            vec![BuildingBehavior::Production {
                inputs: vec![(material("fish"), 3)],
                outputs: vec![(material("smoked_fish"), 3)],
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
        needed_resources: HashMap::from([(material("monument_components"), 3)]),
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
    assert_eq!(report.inventory.opening_stock[&material("fish")], 3);
    assert_eq!(report.inventory.production_consumed[&material("fish")], 3);
    assert_eq!(report.inventory.produced[&material("smoked_fish")], 3);
    assert_eq!(
        report.inventory.population_consumed[&material("smoked_fish")],
        2
    );
    assert!(report.consumption.missing_resources.is_empty());
    assert_eq!(
        report.inventory.project_consumed[&material("monument_components")],
        3
    );
    assert!(report.projects[0].result.is_ok());
    assert_eq!(settlement.inventory().amount(material("smoked_fish")), 3);
    assert_eq!(
        settlement
            .inventory()
            .amount(material("monument_components")),
        1
    );
    assert!(report.inventory.is_balanced());
}
