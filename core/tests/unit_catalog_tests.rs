//! Military foundations: the unit catalogue loaded from `units.toml`
//! (gameplay specification §10.1) and the province graph armies move on
//! (§11.1).
mod common;
use common::material;
use simulation_core::economy::ResourceCatalog;
use simulation_core::military::{UnitCatalog, UnitCatalogError, UnitTypeId};
use simulation_core::world::terrain::area::Area;
use simulation_core::world::{Map, Position, Province, World, WorldError};
use std::collections::BTreeMap;
use std::sync::Arc;

const UNIT_TOML: &str = include_str!("../../units.toml");

fn id(id: &str) -> UnitTypeId {
    UnitTypeId::new(id).unwrap()
}

fn parse(source: &str) -> Result<UnitCatalog, UnitCatalogError> {
    UnitCatalog::from_toml(source, ResourceCatalog::standard())
}

#[test]
fn the_bundled_catalogue_loads_from_disk_with_the_specification_table() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../units.toml");
    let catalog = UnitCatalog::load(path, ResourceCatalog::standard()).unwrap();
    assert_eq!(&catalog, UnitCatalog::standard());
    // (id, attack, defense, movement, gold upkeep per soldier)
    let table: Vec<_> = catalog
        .definitions()
        .iter()
        .map(|unit| {
            (
                unit.id().as_str(),
                unit.attack(),
                unit.defense(),
                unit.movement(),
                unit.gold_upkeep(),
            )
        })
        .collect();
    assert_eq!(
        table,
        vec![
            ("levy", 1, 1, 1, 1),
            ("spearmen", 2, 3, 1, 1),
            ("archers", 3, 1, 1, 1),
            ("cavalry", 3, 2, 2, 2),
            ("siege_engines", 1, 0, 1, 2),
        ]
    );
    let equipment = |unit: &str| catalog.get(&id(unit)).unwrap().equipment().clone();
    assert!(equipment("levy").is_empty());
    assert_eq!(
        equipment("spearmen"),
        BTreeMap::from([(material("bronze"), 25), (material("planks"), 25)])
    );
    assert_eq!(
        equipment("archers"),
        BTreeMap::from([(material("planks"), 50), (material("tools"), 10)])
    );
    assert_eq!(
        equipment("cavalry"),
        BTreeMap::from([(material("cattle"), 25), (material("steel"), 25)])
    );
    assert_eq!(
        equipment("siege_engines"),
        BTreeMap::from([(material("planks"), 50), (material("tools"), 25)])
    );
    for unit in catalog.definitions() {
        assert_eq!(unit.food_upkeep(), 1, "{}", unit.id());
    }
    let rules = catalog.rules();
    assert_eq!(rules.soldiers_per_unit, 50);
    assert_eq!(rules.conscription_share, 0.1);
    assert_eq!(rules.recruitment_turns, 2);
    assert_eq!((rules.initial_morale, rules.max_morale), (100, 100));
    assert_eq!(catalog.garrison_unit().id(), &id("levy"));
    assert_eq!(
        catalog
            .get(&id("siege_engines"))
            .unwrap()
            .wall_bonus_reduction(),
        0.25
    );
}

#[test]
fn advantages_form_the_triangle_of_the_specification() {
    let catalog = UnitCatalog::standard();
    let beats = |a: &str, b: &str| catalog.has_advantage(&id(a), &id(b));
    assert!(beats("spearmen", "cavalry"));
    assert!(beats("cavalry", "archers"));
    assert!(beats("archers", "spearmen"));
    assert!(!beats("cavalry", "spearmen"));
    assert!(!beats("levy", "levy"));
    assert!(!beats("unknown", "levy"));
    assert_eq!(catalog.rules().advantage_multiplier, 1.5);
}

#[test]
fn invalid_catalogues_are_rejected_when_loading() {
    let unit = |extra: &str| {
        format!(
            "{UNIT_TOML}\n[[units]]\nid = \"knights\"\nname = \"Knights\"\nattack = 4\n\
             defense = 4\nmovement = 2\ngold_upkeep = 3\nfood_upkeep = 1\n{extra}\n"
        )
    };
    assert!(parse(&unit("")).is_ok());
    assert!(matches!(
        parse(&unit("").replace("id = \"knights\"", "id = \"Knights\"")),
        Err(UnitCatalogError::InvalidIdentifier(_))
    ));
    assert!(matches!(
        parse(&unit("").replace("id = \"knights\"", "id = \"levy\"")),
        Err(UnitCatalogError::DuplicateUnit(_))
    ));
    assert!(matches!(
        parse(&unit("").replace("name = \"Knights\"", "name = \" \"")),
        Err(UnitCatalogError::EmptyName(_))
    ));
    assert!(matches!(
        parse(&unit("").replace(
            "movement = 2\ngold_upkeep = 3",
            "movement = 0\ngold_upkeep = 3"
        )),
        Err(UnitCatalogError::ZeroMovement(_))
    ));
    assert!(matches!(
        parse(&unit("equipment = { gold = 5 }")),
        Err(UnitCatalogError::UnknownEquipment { .. })
    ));
    assert!(matches!(
        parse(&unit("equipment = { steel = 0 }")),
        Err(UnitCatalogError::ZeroEquipment { .. })
    ));
    for against in ["dragons", "knights"] {
        assert!(
            matches!(
                parse(&unit(&format!("advantage_against = [\"{against}\"]"))),
                Err(UnitCatalogError::InvalidAdvantage { .. })
            ),
            "{against}"
        );
    }
    assert!(matches!(
        parse(&unit("wall_bonus_reduction = 1.5")),
        Err(UnitCatalogError::InvalidWallBonusReduction(_))
    ));
    assert!(matches!(
        parse(&unit("speed = 3")),
        Err(UnitCatalogError::Parse(_))
    ));
    assert!(matches!(
        parse(&UNIT_TOML.replace("garrison_unit = \"levy\"", "garrison_unit = \"militia\"")),
        Err(UnitCatalogError::UnknownGarrisonUnit(_))
    ));
    for (from, to) in [
        ("soldiers_per_unit = 50", "soldiers_per_unit = 0"),
        ("initial_morale = 100", "initial_morale = 101"),
        ("conscription_share = 0.1", "conscription_share = 1.5"),
        ("strength_variance = 0.1", "strength_variance = -0.1"),
        ("advantage_multiplier = 1.5", "advantage_multiplier = 0.0"),
    ] {
        assert!(
            matches!(
                parse(&UNIT_TOML.replace(from, to)),
                Err(UnitCatalogError::InvalidRules(_))
            ),
            "{to}"
        );
    }
    // Equipment names resolve through the resource catalogue, aliases included.
    let charcoal = parse(&unit("equipment = { charcoal = 5 }")).unwrap();
    assert_eq!(
        charcoal.get(&id("knights")).unwrap().equipment(),
        &BTreeMap::from([(material("coal"), 5)])
    );
}

/// Provinces 1 | 2 | 3 in a row, three tiles each.
fn world() -> World {
    World::new(
        Map::new(
            9,
            1,
            vec![Area::new(0.5, 0.5); 9],
            vec![1, 1, 1, 2, 2, 2, 3, 3, 3],
        ),
        [1, 2, 3].into_iter().map(Province::new).collect(),
    )
}

#[test]
fn the_world_keeps_its_province_graph_up_to_date() {
    let mut world = world();
    let graph = world.province_graph();
    assert_eq!(graph.neighbors(2).collect::<Vec<_>>(), vec![1, 3]);
    assert!(!graph.are_neighbors(1, 3));
    assert_eq!(graph.route(1, 3, |_| true), Some(vec![2, 3]));

    // Flooding the border of 1 and 2 separates them.
    world
        .set_tile_terrain(Position { x: 3, y: 0 }, Area::new(0.0, 0.5))
        .unwrap();
    assert!(!world.province_graph().are_neighbors(1, 2));
    assert_eq!(world.province_graph().route(1, 3, |_| true), None);

    // Province 1 grows across the water's edge to touch 2 again.
    world
        .set_tile_terrain(Position { x: 3, y: 0 }, Area::new(0.5, 0.5))
        .unwrap();
    world.set_tile_province(Position { x: 3, y: 0 }, 1).unwrap();
    world.set_tile_province(Position { x: 4, y: 0 }, 1).unwrap();
    world.set_tile_province(Position { x: 5, y: 0 }, 1).unwrap();
    assert!(world.province_graph().are_neighbors(1, 3));
    assert_eq!(world.province_graph().neighbors(2).count(), 0);
}

#[test]
fn a_world_uses_its_own_unit_catalogue_if_the_equipment_exists() {
    let mut world = world();
    assert_eq!(world.unit_catalog(), UnitCatalog::standard());
    let custom =
        parse(&UNIT_TOML.replace("soldiers_per_unit = 50", "soldiers_per_unit = 20")).unwrap();
    world.set_unit_catalog(Arc::new(custom)).unwrap();
    assert_eq!(world.unit_catalog().rules().soldiers_per_unit, 20);

    // A world whose resources lack bronze cannot use spearmen.
    let resources = ResourceCatalog::from_toml(
        "[[resources]]\nid = \"wood\"\nname = \"Wood\"\ntier = 1\n\
         category = \"construction_material\"\n",
    )
    .unwrap();
    let mut world = World::with_resource_catalog(
        Map::new(1, 1, vec![Area::new(0.5, 0.5)], vec![1]),
        vec![Province::new(1)],
        Arc::new(resources),
    );
    assert_eq!(
        world.set_unit_catalog(Arc::new(UnitCatalog::standard().clone())),
        Err(WorldError::UnitEquipmentUnknown(id("spearmen")))
    );
}
