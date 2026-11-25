//! Ordered agent decision execution against the authoritative world.
use crate::test_support::material;
use simulation_core::buildings::Building;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::economy::storage_order::StorageOrderError;
use simulation_core::economy::{ResourceCatalog, ResourceType};
use simulation_core::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use simulation_core::settlement::Settlement;
use simulation_core::settlement::decision::{
    ActionEffect, ActionError, ActionKind, ActionOutcome, DecisionOutcome, OfferSide, OfferSpec,
    RejectedAction, SettlementAction, SettlementDecision,
};
use simulation_core::settlement::projects::{ProjectCost, ProjectResult};
use simulation_core::trade::offer::TradeOffer;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::territory::TileOccupancy;
use simulation_core::world::{Map, Position, Province, World, WorldError, terrain::area::Area};
use std::collections::HashMap;
use std::sync::Arc;

const ME: SettlementId = SettlementId(1);
const NEIGHBOUR: SettlementId = SettlementId(2);
const FARM: BuildingId = BuildingId(1);
const NEIGHBOUR_FARM: BuildingId = BuildingId(9);
const NEIGHBOUR_PROJECT: ProjectId = ProjectId(5);

fn at(x: i32, y: i32) -> Position {
    Position { x, y }
}

fn definition(id: &str, max_workers: u32, wood: u32, turns: u32) -> BuildingDefinition {
    BuildingDefinition::new(
        id,
        id,
        1,
        max_workers,
        vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(material("grain"), 2)],
        }],
        ProjectCost {
            needed_resources: HashMap::from([(material("wood"), wood)]),
            turn_cost: turns,
        },
    )
    .unwrap()
}

/// Map (x right, y down), provinces by ID; `~` is ocean:
/// ```text
/// y0: 10 10 20 30
/// y1: 10 ~10 20 30
/// ```
/// Settlement 1 owns province 10, settlement 2 owns 20, province 30 is empty.
/// Settlement 1: population 5, Wood 10, Grain 3, gold 20, a farm at (0,0).
fn world() -> World {
    world_with_farm_load(0.0)
}

fn world_with_farm_load(load: f32) -> World {
    let mut terrain = vec![Area::new(0.5, 0.5); 8];
    terrain[5] = Area::new(0.0, 0.5);
    let mut world = World::new(
        Map::new(4, 2, terrain, vec![10, 10, 20, 30, 10, 10, 20, 30]),
        vec![Province::new(10), Province::new(20), Province::new(30)],
    );
    let mut catalog = BuildingCatalog::default();
    catalog.register(definition("farm", 4, 2, 3));
    catalog.register(definition("big_farm", 6, 5, 4));
    world.set_building_catalog(catalog);

    let mut me = Settlement::new(ME, 100, 5, Arc::default());
    me.deposit(material("wood"), 10);
    me.deposit(material("grain"), 3);
    me.deposit_gold(20);
    world.add_settlement(10, me).unwrap();
    world
        .add_settlement(20, Settlement::new(NEIGHBOUR, 100, 3, Arc::default()))
        .unwrap();

    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    let mut finder = PathFinder::new(4, 2);
    let mut building = Building::new(FARM, at(0, 0), farm.clone());
    building.set_target_load(load).unwrap();
    world.add_building(ME, building, &mut finder).unwrap();
    world
        .add_building(
            NEIGHBOUR,
            Building::new(NEIGHBOUR_FARM, at(2, 0), farm.clone()),
            &mut finder,
        )
        .unwrap();
    world
        .queue_construction(
            NEIGHBOUR,
            NEIGHBOUR_PROJECT,
            at(2, 1),
            farm,
            ProjectCost::default(),
        )
        .unwrap();
    world
}

fn apply(world: &mut World, actions: Vec<SettlementAction>) -> DecisionOutcome {
    world
        .apply_decision(&SettlementDecision::new(ME, actions))
        .unwrap()
}

fn rejections(result: &DecisionOutcome) -> Vec<RejectedAction> {
    result
        .results
        .iter()
        .filter_map(|result| match &result.outcome {
            ActionOutcome::Applied { .. } => None,
            ActionOutcome::Rejected { error } => Some(RejectedAction {
                index: result.index,
                kind: result.kind,
                error: error.clone(),
            }),
        })
        .collect()
}

fn errors(result: &DecisionOutcome) -> Vec<(usize, ActionError)> {
    rejections(result)
        .into_iter()
        .map(|rejection| (rejection.index, rejection.error))
        .collect()
}

fn accepted_indices(result: &DecisionOutcome) -> Vec<usize> {
    result
        .results
        .iter()
        .filter(|result| result.is_applied())
        .map(|result| result.index)
        .collect()
}

fn effect(result: &DecisionOutcome, index: usize) -> &ActionEffect {
    match &result.results[index].outcome {
        ActionOutcome::Applied { effect } => effect,
        other => panic!("{other:?}"),
    }
}

fn construct(building_type: &str, position: Position) -> SettlementAction {
    SettlementAction::Construct {
        building_type: BuildingTypeId::new(building_type),
        position,
    }
}

fn workers(building_id: BuildingId, workers: u32) -> SettlementAction {
    SettlementAction::SetBuildingWorkers {
        building_id,
        workers,
    }
}

fn offer(resource: ResourceType, side: OfferSide, units: u32, price: u32) -> OfferSpec {
    OfferSpec {
        resource,
        side,
        units,
        price_per_unit: price,
    }
}

fn caravan(receiver_id: SettlementId, offers: Vec<OfferSpec>) -> SettlementAction {
    SettlementAction::OrderCaravan {
        receiver_id,
        offers,
    }
}

fn publish(offers: Vec<OfferSpec>) -> SettlementAction {
    SettlementAction::PublishMarketOffers { offers }
}

/// Adds a second farm of settlement 1 at (0,1) and returns its ID.
fn second_farm(world: &mut World) -> BuildingId {
    second_farm_with_load(world, 0.0)
}

fn second_farm_with_load(world: &mut World, load: f32) -> BuildingId {
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    let mut building = Building::new(BuildingId(2), at(0, 1), farm);
    building.set_target_load(load).unwrap();
    world
        .add_building(ME, building, &mut PathFinder::new(4, 2))
        .unwrap();
    BuildingId(2)
}

#[test]
fn construction_and_upgrade_use_catalog_costs_and_new_project_ids() {
    let mut world = world();
    let result = apply(
        &mut world,
        vec![
            construct("farm", at(1, 0)),
            SettlementAction::Upgrade {
                building_id: FARM,
                building_type: BuildingTypeId::new("big_farm"),
            },
        ],
    );
    assert_eq!(
        [effect(&result, 0), effect(&result, 1)],
        [
            &ActionEffect::ProjectQueued {
                project_id: ProjectId(1)
            },
            &ActionEffect::ProjectQueued {
                project_id: ProjectId(2)
            },
        ]
    );
    let projects = world.settlement(ME).unwrap().active_projects();
    match projects[0].result() {
        ProjectResult::ConstructBuilding {
            position,
            definition,
        } => {
            assert_eq!(*position, at(1, 0));
            assert_eq!(definition.id(), "farm");
        }
        other => panic!("{other:?}"),
    }
    match projects[1].result() {
        ProjectResult::UpgradeBuilding {
            target_instance_id,
            new_definition,
        } => {
            assert_eq!(*target_instance_id, FARM);
            assert_eq!(new_definition.id(), "big_farm");
        }
        other => panic!("{other:?}"),
    }
    for (project, wood, turns) in [(&projects[0], 2, 3), (&projects[1], 5, 4)] {
        let cost = project.remaining_cost();
        assert_eq!(
            cost.needed_resources,
            HashMap::from([(material("wood"), wood)])
        );
        assert_eq!(cost.turn_cost, turns);
    }
}

#[test]
fn project_ids_continue_after_earlier_and_canceled_projects() {
    let mut world = world();
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    world
        .queue_construction(ME, ProjectId(7), at(1, 0), farm, ProjectCost::default())
        .unwrap();
    world.cancel_project(ME, ProjectId(7)).unwrap();
    let result = apply(&mut world, vec![construct("farm", at(1, 0))]);
    assert_eq!(
        effect(&result, 0),
        &ActionEffect::ProjectQueued {
            project_id: ProjectId(8),
        }
    );
}

#[test]
fn construction_sites_follow_placement_and_ownership_rules() {
    let mut world = world();
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    world
        .queue_construction(ME, ProjectId(3), at(0, 1), farm, ProjectCost::default())
        .unwrap();
    let result = apply(
        &mut world,
        vec![
            construct("castle", at(1, 0)),
            construct("farm", at(9, 9)),
            construct("farm", at(-1, 0)),
            construct("farm", at(2, 0)),
            construct("farm", at(3, 0)),
            construct("farm", at(1, 1)),
            construct("farm", at(0, 0)),
            construct("farm", at(0, 1)),
        ],
    );
    assert!(accepted_indices(&result).is_empty());
    assert_eq!(
        errors(&result),
        vec![
            (
                0,
                ActionError::UnknownBuildingType {
                    building_type: BuildingTypeId::new("castle")
                }
            ),
            (1, ActionError::InvalidPosition { position: at(9, 9) }),
            (
                2,
                ActionError::InvalidPosition {
                    position: at(-1, 0)
                }
            ),
            (3, ActionError::OutsideOwnProvince { position: at(2, 0) }),
            (4, ActionError::OutsideOwnProvince { position: at(3, 0) }),
            (5, ActionError::OceanTile { position: at(1, 1) }),
            (6, ActionError::TileOccupied { position: at(0, 0) }),
            (7, ActionError::TileOccupied { position: at(0, 1) }),
        ]
    );
}

#[test]
fn assets_of_other_settlements_cannot_be_targeted() {
    let mut world = world();
    let result = apply(
        &mut world,
        vec![
            SettlementAction::Upgrade {
                building_id: NEIGHBOUR_FARM,
                building_type: BuildingTypeId::new("big_farm"),
            },
            workers(NEIGHBOUR_FARM, 1),
            SettlementAction::CancelProject {
                project_id: NEIGHBOUR_PROJECT,
            },
        ],
    );
    assert_eq!(
        errors(&result),
        vec![
            (
                0,
                ActionError::UnknownBuilding {
                    building_id: NEIGHBOUR_FARM
                }
            ),
            (
                1,
                ActionError::UnknownBuilding {
                    building_id: NEIGHBOUR_FARM
                }
            ),
            (
                2,
                ActionError::UnknownProject {
                    project_id: NEIGHBOUR_PROJECT
                }
            ),
        ]
    );
}

#[test]
fn worker_requests_respect_building_capacity_and_population() {
    let mut world = world();
    let second = second_farm(&mut world);
    let result = apply(
        &mut world,
        vec![
            workers(FARM, 5),
            workers(FARM, 4),
            workers(second, 2),
            workers(second, 1),
        ],
    );
    assert_eq!(
        errors(&result),
        vec![
            (
                0,
                ActionError::WorkersAboveCapacity {
                    building_id: FARM,
                    workers: 5,
                    max_workers: 4
                }
            ),
            (
                2,
                ActionError::InsufficientWorkers {
                    requested: 6,
                    available: 5
                }
            ),
        ]
    );
    assert_eq!(accepted_indices(&result), vec![1, 3]);
    for (building, expected) in world.settlement(ME).unwrap().buildings().zip([4, 1]) {
        assert_eq!(building.requested_workers(), expected);
    }
}

#[test]
fn staff_can_be_reduced_when_buildings_already_ask_for_too_many() {
    let mut world = world_with_farm_load(1.0);
    let second = second_farm_with_load(&mut world, 1.0);
    // 8 requested workers for a population of 5.
    let result = apply(&mut world, vec![workers(FARM, 3), workers(second, 4)]);
    assert!(errors(&result).is_empty(), "{result:?}");
    let result = apply(&mut world, vec![workers(FARM, 0), workers(second, 4)]);
    assert!(errors(&result).is_empty(), "{result:?}");
}

#[test]
fn upgrades_need_an_own_building_a_new_type_and_no_running_upgrade() {
    let mut world = world();
    let second = second_farm(&mut world);
    let big_farm = world.building_catalog().get_blueprint("big_farm").unwrap();
    world
        .queue_upgrade(ME, ProjectId(3), second, big_farm, ProjectCost::default())
        .unwrap();
    let upgrade = |building_id, building_type: &str| SettlementAction::Upgrade {
        building_id,
        building_type: BuildingTypeId::new(building_type),
    };
    let result = apply(
        &mut world,
        vec![
            upgrade(FARM, "farm"),
            upgrade(second, "big_farm"),
            upgrade(BuildingId(42), "big_farm"),
            upgrade(FARM, "castle"),
            upgrade(FARM, "big_farm"),
        ],
    );
    assert_eq!(
        errors(&result),
        vec![
            (0, ActionError::SameBuildingType { building_id: FARM }),
            (
                1,
                ActionError::BuildingAlreadyUpgrading {
                    building_id: second
                }
            ),
            (
                2,
                ActionError::UnknownBuilding {
                    building_id: BuildingId(42)
                }
            ),
            (
                3,
                ActionError::UnknownBuildingType {
                    building_type: BuildingTypeId::new("castle")
                }
            ),
        ]
    );
    assert_eq!(accepted_indices(&result), vec![4]);
}

#[test]
fn only_active_own_projects_can_be_canceled() {
    let mut world = world();
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    world
        .queue_construction(ME, ProjectId(3), at(1, 0), farm, ProjectCost::default())
        .unwrap();
    let cancel = |id| SettlementAction::CancelProject {
        project_id: ProjectId(id),
    };
    let result = apply(&mut world, vec![cancel(4), cancel(3)]);
    assert_eq!(
        errors(&result),
        vec![(
            0,
            ActionError::UnknownProject {
                project_id: ProjectId(4)
            }
        )]
    );
    assert_eq!(
        effect(&result, 1),
        &ActionEffect::ProjectCanceled {
            project_id: ProjectId(3)
        }
    );
    assert!(world.settlement(ME).unwrap().active_projects().is_empty());
}

#[test]
fn storage_orders_use_the_engine_rule() {
    let mut world = world();
    let order = |order: Vec<ResourceType>| SettlementAction::SetStorageOrder { order };
    for (materials, error) in [
        (
            vec![material("wood"), material("wood"), material("grain")],
            StorageOrderError::DuplicateMaterial(material("wood")),
        ),
        (
            vec![material("wood"), material("stone")],
            StorageOrderError::MissingMaterial(material("grain")),
        ),
        (
            vec![
                ResourceType::Labour,
                material("wood"),
                material("stone"),
                material("grain"),
            ],
            StorageOrderError::LabourIsNotMaterial,
        ),
    ] {
        let result = apply(&mut world, vec![order(materials)]);
        assert_eq!(
            errors(&result),
            vec![(0, ActionError::InvalidStorageOrder { error })]
        );
    }
    let result = apply(
        &mut world,
        vec![order(
            [material("grain"), material("stone"), material("wood")]
                .into_iter()
                .chain(ResourceCatalog::standard().materials()[3..].iter().cloned())
                .collect(),
        )],
    );
    assert!(errors(&result).is_empty(), "{result:?}");
}

#[test]
fn market_offers_are_checked_and_resolved_to_fresh_offers() {
    let mut world = world();
    for (offers, error) in [
        (
            vec![offer(ResourceType::Labour, OfferSide::Sell, 1, 1)],
            ActionError::LabourIsNotTradable,
        ),
        (
            vec![offer(material("wood"), OfferSide::Sell, 0, 1)],
            ActionError::ZeroUnits {
                resource: material("wood"),
            },
        ),
        (
            vec![
                offer(material("wood"), OfferSide::Sell, 2, 1),
                offer(material("wood"), OfferSide::Buy, 2, 1),
            ],
            ActionError::DuplicateOffer {
                resource: material("wood"),
            },
        ),
    ] {
        let result = apply(&mut world, vec![publish(offers)]);
        assert_eq!(errors(&result), vec![(0, error)]);
    }
    // Market offers are not limited by stock: each turn reserves what is available.
    let result = apply(
        &mut world,
        vec![publish(vec![
            offer(material("wood"), OfferSide::Sell, 500, 2),
            offer(material("stone"), OfferSide::Buy, 4, 3),
        ])],
    );
    assert_eq!(effect(&result, 0), &ActionEffect::MarketOffersPublished);
    assert_eq!(
        world.settlement(ME).unwrap().market_offers(),
        &HashMap::from([
            (material("wood"), TradeOffer::sell(500, 2)),
            (material("stone"), TradeOffer::buy(4, 3))
        ])
    );
    let result = apply(&mut world, vec![SettlementAction::ClearMarketOffers]);
    assert_eq!(effect(&result, 0), &ActionEffect::MarketOffersCleared);
    assert!(world.settlement(ME).unwrap().market_offers().is_empty());
}

#[test]
fn caravans_need_a_receiver_and_enough_stock_and_gold_together() {
    let mut world = world();
    let result = apply(
        &mut world,
        vec![
            caravan(NEIGHBOUR, vec![]),
            caravan(ME, vec![offer(material("wood"), OfferSide::Sell, 1, 1)]),
            caravan(
                SettlementId(99),
                vec![offer(material("wood"), OfferSide::Sell, 1, 1)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(ResourceType::Labour, OfferSide::Sell, 1, 1)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(material("wood"), OfferSide::Sell, 6, 1)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(material("wood"), OfferSide::Sell, 6, 1)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(material("grain"), OfferSide::Buy, 4, 3)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(material("stone"), OfferSide::Buy, 5, 2)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(material("wood"), OfferSide::Sell, 4, 1)],
            ),
        ],
    );
    assert_eq!(
        errors(&result),
        vec![
            (0, ActionError::EmptyOrder),
            (1, ActionError::TradeWithSelf),
            (
                2,
                ActionError::UnknownSettlement {
                    settlement_id: SettlementId(99)
                }
            ),
            (3, ActionError::LabourIsNotTradable),
            // What is left for this caravan after the earlier ones.
            (
                5,
                ActionError::InsufficientStock {
                    resource: material("wood"),
                    requested: 6,
                    available: 4
                }
            ),
            (
                7,
                ActionError::InsufficientGold {
                    requested: 10,
                    available: 8
                }
            ),
        ]
    );
    // Several caravans to the same receiver are allowed while stock lasts.
    assert_eq!(accepted_indices(&result), vec![4, 6, 8]);
}

/// Settlement 1 publishes market offers before the decision is validated.
fn world_with_market(offers: Vec<OfferSpec>) -> World {
    let mut world = world();
    let decision = SettlementDecision::new(ME, vec![publish(offers)]);
    let outcome = world.apply_decision(&decision).unwrap();
    assert!(outcome.results[0].is_applied());
    world
}

#[test]
fn caravans_get_only_what_the_published_market_leaves() {
    // Stock: Wood 10, gold 20. The market reserves 7 Wood and 4 Ã— 3 = 12 gold first.
    let mut world = world_with_market(vec![
        offer(material("wood"), OfferSide::Sell, 7, 1),
        offer(material("stone"), OfferSide::Buy, 4, 3),
    ]);
    let result = apply(
        &mut world,
        vec![
            caravan(
                NEIGHBOUR,
                vec![offer(material("wood"), OfferSide::Sell, 4, 1)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(material("wood"), OfferSide::Sell, 3, 1)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(material("grain"), OfferSide::Buy, 3, 3)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(material("grain"), OfferSide::Buy, 4, 2)],
            ),
        ],
    );
    assert_eq!(
        errors(&result),
        vec![
            (
                0,
                ActionError::InsufficientStock {
                    resource: material("wood"),
                    requested: 4,
                    available: 3
                }
            ),
            (
                2,
                ActionError::InsufficientGold {
                    requested: 9,
                    available: 8
                }
            ),
        ]
    );
    assert_eq!(accepted_indices(&result), vec![1, 3]);
}

#[test]
fn a_market_action_in_the_same_decision_changes_what_caravans_get() {
    let mut world = world_with_market(vec![offer(material("wood"), OfferSide::Sell, 10, 1)]);
    let caravan_wood = |units| {
        caravan(
            NEIGHBOUR,
            vec![offer(material("wood"), OfferSide::Sell, units, 1)],
        )
    };
    // Clearing or shrinking the market first frees its stock for caravans.
    let result = apply(
        &mut world,
        vec![SettlementAction::ClearMarketOffers, caravan_wood(10)],
    );
    assert!(errors(&result).is_empty(), "{result:?}");
    let mut world = world_with_market(vec![offer(material("wood"), OfferSide::Sell, 10, 1)]);
    let result = apply(
        &mut world,
        vec![
            publish(vec![offer(material("wood"), OfferSide::Sell, 6, 1)]),
            caravan_wood(4),
            caravan_wood(1),
        ],
    );
    assert_eq!(
        errors(&result),
        vec![(
            2,
            ActionError::InsufficientStock {
                resource: material("wood"),
                requested: 1,
                available: 0
            }
        )]
    );
}

#[test]
fn a_later_market_action_cannot_take_what_caravans_were_promised() {
    let mut world = world();
    let result = apply(
        &mut world,
        vec![
            caravan(
                NEIGHBOUR,
                vec![offer(material("wood"), OfferSide::Sell, 6, 1)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(material("grain"), OfferSide::Buy, 5, 3)],
            ),
            publish(vec![offer(material("wood"), OfferSide::Sell, 5, 1)]),
            publish(vec![offer(material("stone"), OfferSide::Buy, 2, 3)]),
            publish(vec![
                offer(material("wood"), OfferSide::Sell, 4, 1),
                offer(material("stone"), OfferSide::Buy, 1, 5),
            ]),
        ],
    );
    assert_eq!(
        errors(&result),
        vec![
            (
                2,
                ActionError::InsufficientStock {
                    resource: material("wood"),
                    requested: 5,
                    available: 4
                }
            ),
            (
                3,
                ActionError::InsufficientGold {
                    requested: 6,
                    available: 5
                }
            ),
        ]
    );
    // Exactly what is left is fine; markets without competing caravans stay unlimited.
    assert_eq!(accepted_indices(&result), vec![0, 1, 4]);
    let mut world = self::world();
    let result = apply(
        &mut world,
        vec![publish(vec![offer(
            material("wood"),
            OfferSide::Sell,
            999,
            1,
        )])],
    );
    assert!(errors(&result).is_empty(), "{result:?}");
}

#[test]
fn caravans_queued_earlier_in_the_turn_count_too() {
    let mut world = world();
    let earlier = SettlementDecision::new(
        ME,
        vec![caravan(
            NEIGHBOUR,
            vec![offer(material("wood"), OfferSide::Sell, 8, 1)],
        )],
    );
    assert!(world.apply_decision(&earlier).unwrap().results[0].is_applied());
    let result = apply(
        &mut world,
        vec![caravan(
            NEIGHBOUR,
            vec![offer(material("wood"), OfferSide::Sell, 3, 1)],
        )],
    );
    assert_eq!(
        errors(&result),
        vec![(
            0,
            ActionError::InsufficientStock {
                resource: material("wood"),
                requested: 3,
                available: 2
            }
        )]
    );
}

#[test]
fn caravans_must_carry_goods_or_gold() {
    let mut world = world();
    let result = apply(
        &mut world,
        vec![
            caravan(
                NEIGHBOUR,
                vec![offer(material("grain"), OfferSide::Buy, 5, 0)],
            ),
            caravan(
                NEIGHBOUR,
                vec![offer(material("grain"), OfferSide::Buy, 5, 1)],
            ),
        ],
    );
    assert_eq!(errors(&result), vec![(0, ActionError::NothingToCarry)]);
    assert_eq!(accepted_indices(&result), vec![1]);
}

#[test]
fn later_actions_on_the_same_target_are_conflicts() {
    let mut world = world();
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    world
        .queue_construction(ME, ProjectId(3), at(0, 1), farm, ProjectCost::default())
        .unwrap();
    let upgrade = SettlementAction::Upgrade {
        building_id: FARM,
        building_type: BuildingTypeId::new("big_farm"),
    };
    let cancel = SettlementAction::CancelProject {
        project_id: ProjectId(3),
    };
    let order = SettlementAction::SetStorageOrder {
        order: [material("grain"), material("stone"), material("wood")]
            .into_iter()
            .chain(ResourceCatalog::standard().materials()[3..].iter().cloned())
            .collect(),
    };
    let result = apply(
        &mut world,
        vec![
            construct("farm", at(1, 0)),
            construct("big_farm", at(1, 0)),
            workers(FARM, 1),
            workers(FARM, 2),
            publish(vec![offer(material("wood"), OfferSide::Sell, 1, 1)]),
            SettlementAction::ClearMarketOffers,
            order.clone(),
            order,
            cancel.clone(),
            cancel,
            upgrade.clone(),
            upgrade,
        ],
    );
    assert_eq!(accepted_indices(&result), vec![0, 2, 4, 6, 8, 10]);
    assert_eq!(
        errors(&result),
        [1, 3, 5, 7, 9, 11]
            .map(|index| (
                index,
                ActionError::Conflict {
                    with_index: index - 1
                }
            ))
            .to_vec()
    );
}

#[test]
fn canceling_a_new_project_releases_its_claim_in_the_same_decision() {
    let mut world = world();
    let site = at(1, 0);
    let upgrade = SettlementAction::Upgrade {
        building_id: FARM,
        building_type: BuildingTypeId::new("big_farm"),
    };
    let result = apply(
        &mut world,
        vec![
            construct("farm", site),
            SettlementAction::CancelProject {
                project_id: ProjectId(1),
            },
            construct("big_farm", site),
            upgrade.clone(),
            SettlementAction::CancelProject {
                project_id: ProjectId(3),
            },
            upgrade,
        ],
    );
    assert_eq!(accepted_indices(&result), vec![0, 1, 2, 3, 4, 5]);
    let projects = world.settlement(ME).unwrap().active_projects();
    assert_eq!(
        projects
            .iter()
            .map(|project| project.id())
            .collect::<Vec<_>>(),
        vec![ProjectId(2), ProjectId(4)]
    );
    assert!(matches!(world.tile_at(site).unwrap().occupancy(),
        TileOccupancy::Construction(reference) if reference.project_id == ProjectId(2)
    ));
}

#[test]
fn rejected_actions_claim_nothing() {
    let mut world = world();
    let result = apply(
        &mut world,
        vec![
            construct("castle", at(1, 0)),
            construct("farm", at(1, 1)),
            construct("farm", at(1, 0)),
            workers(FARM, 9),
            workers(FARM, 2),
        ],
    );
    assert_eq!(accepted_indices(&result), vec![2, 4]);
    // The rejected constructions did not use up a project ID.
    assert_eq!(
        effect(&result, 2),
        &ActionEffect::ProjectQueued {
            project_id: ProjectId(1),
        }
    );
}

#[test]
fn every_action_is_either_accepted_or_rejected_in_order_and_deterministically() {
    let mut world = world();
    let actions = vec![
        workers(FARM, 2),
        construct("farm", at(3, 0)),
        caravan(
            NEIGHBOUR,
            vec![offer(material("wood"), OfferSide::Sell, 3, 1)],
        ),
        SettlementAction::CancelProject {
            project_id: ProjectId(1),
        },
        SettlementAction::ClearMarketOffers,
    ];
    let result = apply(&mut world, actions.clone());
    assert_eq!(accepted_indices(&result), vec![0, 2, 4]);
    assert_eq!(
        rejections(&result),
        vec![
            RejectedAction {
                index: 1,
                kind: ActionKind::Construct,
                error: ActionError::OutsideOwnProvince { position: at(3, 0) }
            },
            RejectedAction {
                index: 3,
                kind: ActionKind::CancelProject,
                error: ActionError::UnknownProject {
                    project_id: ProjectId(1)
                }
            },
        ]
    );
    let again = apply(&mut self::world(), actions);
    assert_eq!(again, result);
}

#[test]
fn rejected_actions_leave_the_world_unchanged() {
    let mut world = world();
    let snapshot = |world: &World| {
        let me = world.settlement(ME).unwrap();
        format!(
            "{:?} {} {:?} {:?} {:?} {:?} {:?} {:?}",
            me.inventory(),
            me.gold(),
            me.active_projects(),
            me.next_project_id(),
            me.buildings()
                .map(Building::target_load)
                .collect::<Vec<_>>(),
            me.storage_order(),
            me.market_offers(),
            me.caravan_orders(),
        )
    };
    let before = snapshot(&world);
    let result = apply(
        &mut world,
        vec![
            construct("farm", at(0, 0)),
            workers(FARM, 5),
            SettlementAction::SetStorageOrder {
                order: vec![material("wood")],
            },
            publish(vec![offer(material("wood"), OfferSide::Sell, 0, 1)]),
            caravan(
                NEIGHBOUR,
                vec![offer(material("wood"), OfferSide::Sell, 11, 1)],
            ),
        ],
    );
    assert_eq!(errors(&result).len(), 5);
    assert_eq!(snapshot(&world), before);
    assert_eq!(
        world.tile_at(at(1, 0)).unwrap().occupancy(),
        TileOccupancy::Empty
    );
}

#[test]
fn a_decision_of_an_unknown_settlement_is_rejected_as_a_whole() {
    let mut world = world();
    let decision = SettlementDecision::new(SettlementId(99), vec![workers(FARM, 1)]);
    assert!(matches!(
        world.apply_decision(&decision),
        Err(WorldError::MissingSettlement(SettlementId(99)))
    ));
}

#[test]
fn rejections_serialize_for_agents() {
    let mut world = world();
    let result = apply(
        &mut world,
        vec![construct("farm", at(0, 0)), workers(FARM, 5)],
    );
    assert_eq!(
        serde_json::to_value(rejections(&result)).unwrap(),
        serde_json::json!([
            {"index": 0, "kind": "construct",
             "error": {"reason": "tile_occupied", "position": {"x": 0, "y": 0}}},
            {"index": 1, "kind": "set_building_workers",
             "error": {"reason": "workers_above_capacity", "building_id": 1,
                       "workers": 5, "max_workers": 4}}
        ])
    );
    let storage = RejectedAction {
        index: 0,
        kind: ActionKind::SetStorageOrder,
        error: ActionError::InvalidStorageOrder {
            error: StorageOrderError::DuplicateMaterial(material("wood")),
        },
    };
    let json = serde_json::to_string(&storage).unwrap();
    assert_eq!(
        serde_json::from_str::<RejectedAction>(&json).unwrap(),
        storage
    );
}
