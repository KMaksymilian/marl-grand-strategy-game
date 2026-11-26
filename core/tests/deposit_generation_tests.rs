mod common;

use common::material;
use simulation_core::economy::{ResourceCatalog, ResourceType};
use simulation_core::ids::SettlementId;
use simulation_core::settlement::Settlement;
use simulation_core::world::generation::deposits::{DepositGenerationConfig, OreDepositRule};
use simulation_core::world::terrain::{area::Area, biome::Biome};
use simulation_core::world::{Map, Position, Province, World};
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn catalog_limits_control_vein_count_and_sizes() {
    let catalog = Arc::new(
        ResourceCatalog::from_toml(
            r#"
        [deposits]
        max_veins = 1
        max_vein_tiles = 4

        [[resources]]
        id = "iron_ore"
        name = "Iron"
        tier = 1
        category = "metal"
        max_vein_tiles = 2

        [[resources]]
        id = "copper_ore"
        name = "Copper"
        tier = 1
        category = "metal"
        "#,
        )
        .unwrap(),
    );
    let scenario = || {
        World::with_resource_catalog(
            Map::new(
                11,
                1,
                [
                    vec![area(Biome::Forest); 5],
                    vec![area(Biome::Ocean)],
                    vec![area(Biome::Desert); 5],
                ]
                .concat(),
                [vec![10; 6], vec![20; 5]].concat(),
            ),
            vec![Province::new(10), Province::new(20)],
            Arc::clone(&catalog),
        )
    };
    let mut configuration = DepositGenerationConfig::for_catalog(67, &catalog);
    for rule in &mut configuration.rules {
        rule.probabilities = [0; 8];
        let biome_index = if rule.resource_id == "iron_ore" { 2 } else { 0 };
        rule.probabilities[biome_index] = 100;
    }
    let capped = scenario().generate_deposits(&configuration).unwrap();
    assert_eq!(capped.veins.len(), 1);
    assert_eq!(
        capped,
        scenario().generate_deposits(&configuration).unwrap()
    );

    configuration.max_veins = 2;
    let report = scenario().generate_deposits(&configuration).unwrap();
    let sizes: HashMap<_, _> = report
        .veins
        .iter()
        .map(|vein| (vein.resource.as_str(), vein.tiles.len()))
        .collect();
    assert_eq!(sizes, HashMap::from([("IronOre", 2), ("CopperOre", 4)]));

    configuration.max_veins = 0;
    let mut disabled = scenario();
    assert!(
        disabled
            .generate_deposits(&configuration)
            .unwrap()
            .veins
            .is_empty()
    );
    assert_eq!(disabled.map().deposits().count(), 0);
}

const LAND_BIOMES: [Biome; 8] = [
    Biome::Desert,
    Biome::Grassland,
    Biome::Forest,
    Biome::Rainforest,
    Biome::Hills,
    Biome::Taiga,
    Biome::Tundra,
    Biome::Snow,
];

fn at(x: i32) -> Position {
    Position { x, y: 0 }
}

fn area(biome: Biome) -> Area {
    match biome {
        Biome::Ocean => Area::new(0.1, 0.45),
        Biome::Desert => Area::new(0.3, 0.3),
        Biome::Grassland => Area::new(0.3, 0.45),
        Biome::Forest => Area::new(0.3, 0.55),
        Biome::Rainforest => Area::new(0.3, 0.7),
        Biome::Hills => Area::new(0.7, 0.3),
        Biome::Taiga => Area::new(0.7, 0.7),
        Biome::Tundra => Area::new(0.9, 0.3),
        Biome::Snow => Area::new(0.9, 0.7),
    }
}

fn world(biomes: &[Biome], territory: Vec<usize>, provinces: &[usize]) -> World {
    World::new(
        Map::new(
            biomes.len(),
            1,
            biomes.iter().copied().map(area).collect(),
            territory,
        ),
        provinces.iter().copied().map(Province::new).collect(),
    )
}

fn config(
    resource_id: &str,
    probabilities: [u8; 8],
    max_vein_tiles: u32,
) -> DepositGenerationConfig {
    DepositGenerationConfig {
        seed: 1234,
        max_vein_tiles,
        max_veins: u32::MAX,
        rules: vec![OreDepositRule {
            resource_id: resource_id.into(),
            probabilities,
        }],
    }
}

fn deposits(world: &World) -> Vec<(Position, ResourceType)> {
    (0..world.map().width)
        .filter_map(|x| {
            let position = at(x as i32);
            world.map().deposit_at(position).map(|ore| (position, ore))
        })
        .collect()
}

#[test]
fn default_generation_requires_canonical_ore_entries() {
    let catalog = ResourceCatalog::from_toml(
        r#"
        [[resources]]
        id = "salt"
        name = "Salt"
        tier = 1
        category = "raw_food"
        aliases = ["iron_ore", "tin_ore"]
        "#,
    )
    .unwrap();

    assert!(
        DepositGenerationConfig::for_catalog(67, &catalog)
            .rules
            .is_empty()
    );
}

#[test]
fn default_ore_chances_match_each_land_biome_and_exclude_ocean() {
    let configuration = DepositGenerationConfig::default();
    for (id, chances) in [
        ("coal", [0, 0, 25, 10, 5, 30, 0, 0]),
        ("iron_ore", [0, 0, 5, 0, 25, 5, 30, 20]),
        ("copper_ore", [30, 0, 0, 0, 20, 0, 5, 0]),
        ("tin_ore", [5, 0, 0, 0, 0, 0, 10, 20]),
    ] {
        let rule = configuration
            .rules
            .iter()
            .find(|rule| rule.resource_id == id)
            .unwrap();
        for (biome, expected) in LAND_BIOMES.into_iter().zip(chances) {
            assert_eq!(rule.chance_for(biome), expected, "{id}, {biome:?}");
        }
        assert_eq!(rule.chance_for(Biome::Ocean), 0);
    }
}

#[test]
fn province_rolls_use_majority_land_biome_and_the_documented_tie_order() {
    for (biomes, expected_veins) in [
        (
            vec![Biome::Desert, Biome::Forest, Biome::Ocean, Biome::Ocean],
            1,
        ),
        (vec![Biome::Desert, Biome::Forest, Biome::Forest], 0),
        (vec![Biome::Ocean, Biome::Ocean], 0),
    ] {
        let mut world = world(&biomes, vec![10; biomes.len()], &[10]);
        let result = world
            .generate_deposits(&config("iron_ore", [100, 0, 0, 0, 0, 0, 0, 0], 1))
            .unwrap();
        assert_eq!(result.veins.len(), expected_veins);
        for vein in result.veins {
            assert_eq!(world.map().province_at(vein.focus), Some(10));
            assert!(world.map().validate_building_position(vein.focus).is_ok());
        }
    }
    for (index, biome) in LAND_BIOMES.into_iter().enumerate() {
        let mut chances = [0; 8];
        chances[index] = 100;
        let mut world = world(&[biome], vec![10], &[10]);
        let report = world
            .generate_deposits(&config("iron_ore", chances, 1))
            .unwrap();
        assert_eq!(report.veins.len(), 1, "{biome:?}");
        assert_eq!(deposits(&world), vec![(at(0), material("iron_ore"))]);
    }
}

#[test]
fn veins_cross_province_boundaries_and_access_follows_tile_ownership() {
    let mut world = world(
        &[
            Biome::Desert,
            Biome::Grassland,
            Biome::Grassland,
            Biome::Grassland,
        ],
        vec![10, 20, 20, 20],
        &[10, 20],
    );
    for (province, id) in [(10, SettlementId(1)), (20, SettlementId(2))] {
        world
            .add_settlement(province, Settlement::new(id, 10, 0, Arc::default()))
            .unwrap();
    }
    let report = world
        .generate_deposits(&config("iron_ore", [100, 0, 0, 0, 0, 0, 0, 0], 4))
        .unwrap();
    assert_eq!(report.veins.len(), 1);
    assert_eq!(report.veins[0].province_id, 10);
    assert_eq!(report.veins[0].focus, at(0));
    assert_eq!(report.veins[0].tiles.len(), 4);
    assert_eq!(
        world.accessible_deposits(SettlementId(1)).unwrap(),
        vec![(at(0), material("iron_ore"))]
    );
    assert_eq!(
        world.accessible_deposits(SettlementId(2)).unwrap(),
        vec![
            (at(1), material("iron_ore")),
            (at(2), material("iron_ore")),
            (at(3), material("iron_ore")),
        ]
    );
}

#[test]
fn water_blocks_growth_and_existing_deposits_are_preserved() {
    let mut island_world = world(
        &[Biome::Desert, Biome::Ocean, Biome::Desert],
        vec![10; 3],
        &[10],
    );
    let report = island_world
        .generate_deposits(&config("iron_ore", [100; 8], 10))
        .unwrap();
    assert_eq!(report.veins.len(), 1);
    assert_eq!(report.veins[0].tiles.len(), 1);
    assert!(island_world.map().deposit_at(at(1)).is_none());
    assert_eq!(deposits(&island_world).len(), 1);

    let mut diagonal_world = World::new(
        Map::new(
            2,
            2,
            vec![
                area(Biome::Desert),
                area(Biome::Ocean),
                area(Biome::Ocean),
                area(Biome::Desert),
            ],
            vec![10; 4],
        ),
        vec![Province::new(10)],
    );
    let report = diagonal_world
        .generate_deposits(&config("iron_ore", [100; 8], 10))
        .unwrap();
    assert_eq!(report.veins.len(), 1);
    assert_eq!(report.veins[0].tiles.len(), 1);

    let mut occupied_world = world(&[Biome::Desert; 3], vec![10; 3], &[10]);
    occupied_world
        .place_deposit(at(0), material("coal"))
        .unwrap();
    occupied_world
        .generate_deposits(&config("iron_ore", [100; 8], 3))
        .unwrap();
    assert_eq!(
        occupied_world.map().deposit_at(at(0)),
        Some(material("coal"))
    );
    assert!(
        deposits(&occupied_world)
            .iter()
            .any(|(_, ore)| ore == &material("iron_ore"))
    );
}

#[test]
fn seeded_rolls_and_focuses_are_stable_across_rule_order_and_vein_sizes() {
    let make_world = || {
        world(
            &[Biome::Forest; 30],
            (0..30).map(|x| x / 10).collect(),
            &[0, 1, 2],
        )
    };
    let mut configuration = config("iron_ore", [100; 8], 1);
    configuration.rules.push(OreDepositRule {
        resource_id: "coal".into(),
        probabilities: [100; 8],
    });
    let mut first = make_world();
    let report = first.generate_deposits(&configuration).unwrap();
    assert_eq!(report.veins.len(), 6);
    let focuses: Vec<_> = report
        .veins
        .iter()
        .map(|vein| (vein.province_id, vein.resource.clone(), vein.focus))
        .collect();
    let mut repeated = make_world();
    repeated.generate_deposits(&configuration).unwrap();
    assert_eq!(deposits(&first), deposits(&repeated));
    configuration.rules.reverse();
    let mut reversed = make_world();
    reversed.generate_deposits(&configuration).unwrap();
    assert_eq!(deposits(&first), deposits(&reversed));
    configuration.max_vein_tiles = 5;
    let mut larger = make_world();
    let report = larger.generate_deposits(&configuration).unwrap();
    assert_eq!(
        focuses,
        report
            .veins
            .iter()
            .map(|vein| (vein.province_id, vein.resource.clone(), vein.focus))
            .collect::<Vec<_>>()
    );
}

#[test]
fn invalid_generation_configuration_is_rejected_before_world_mutation() {
    let mut invalid = vec![
        config("iron_ore", [100; 8], 0),
        config("iron_ore", [101; 8], 2),
        config("unknown_ore", [100; 8], 2),
    ];
    let mut duplicate = config("coal", [100; 8], 2);
    duplicate.rules.push(OreDepositRule {
        resource_id: "charcoal".into(),
        probabilities: [100; 8],
    });
    invalid.push(duplicate);
    for configuration in invalid {
        let mut world = world(&[Biome::Forest; 3], vec![10; 3], &[10]);
        world.place_deposit(at(0), material("coal")).unwrap();
        let mut settlement = Settlement::new(SettlementId(1), 10, 0, Arc::default());
        settlement.deposit(material("wood"), 7);
        world.add_settlement(10, settlement).unwrap();
        let before = deposits(&world);
        assert!(world.generate_deposits(&configuration).is_err());
        assert_eq!(deposits(&world), before);
        assert_eq!(
            world
                .settlement(SettlementId(1))
                .unwrap()
                .inventory()
                .amount(material("wood")),
            7
        );
    }
    let mut world = World::new(
        Map::new(3, 1, vec![area(Biome::Forest)], vec![10]),
        vec![Province::new(10)],
    );
    assert!(
        world
            .generate_deposits(&config("iron_ore", [100; 8], 2))
            .is_err()
    );
}
