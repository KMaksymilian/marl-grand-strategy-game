//! Agent decision validation against the authoritative world (read-only).
use simulation_core::buildings::Building;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::economy::ResourceType::{self, Grain, Labour, Stone, Wood};
use simulation_core::economy::storage_order::StorageOrderError;
use simulation_core::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use simulation_core::settlement::Settlement;
use simulation_core::settlement::command::SettlementCommand;
use simulation_core::settlement::decision::{
    ActionEffect, ActionError, ActionKind, OfferSide, OfferSpec, RejectedAction, SettlementAction,
    SettlementDecision,
};
use simulation_core::settlement::projects::ProjectCost;
use simulation_core::trade::offer::TradeOffer;
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::territory::TileOccupancy;
use simulation_core::world::{
    Map, Position, Province, ValidatedDecision, World, WorldError, terrain::area::Area,
};
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
            outputs: vec![(Grain, 2)],
        }],
        ProjectCost {
            needed_resources: HashMap::from([(Wood, wood)]),
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
    me.deposit(Wood, 10);
    me.deposit(Grain, 3);
    me.deposit_gold(20);
    world.add_settlement(10, me).unwrap();
    world
        .add_settlement(20, Settlement::new(NEIGHBOUR, 100, 3, Arc::default()))
        .unwrap();

    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    let mut finder = PathFinder::new(4, 2);
    world
        .add_building(ME, Building::new(FARM, at(0, 0), farm.clone()), &mut finder)
        .unwrap();
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

fn validate(world: &World, actions: Vec<SettlementAction>) -> ValidatedDecision {
    world
        .validate_decision(&SettlementDecision::new(ME, actions))
        .unwrap()
}

fn errors(result: &ValidatedDecision) -> Vec<(usize, ActionError)> {
    result
        .rejected
        .iter()
        .map(|rejection| (rejection.index, rejection.error.clone()))
        .collect()
}

fn accepted_indices(result: &ValidatedDecision) -> Vec<usize> {
    result.accepted.iter().map(|action| action.index).collect()
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
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    world
        .add_building(
            ME,
            Building::new(BuildingId(2), at(0, 1), farm),
            &mut PathFinder::new(4, 2),
        )
        .unwrap();
    BuildingId(2)
}

#[test]
fn construction_and_upgrade_use_catalog_costs_and_new_project_ids() {
    let world = world();
    let result = validate(
        &world,
        vec![
            construct("farm", at(1, 0)),
            SettlementAction::Upgrade {
                building_id: FARM,
                building_type: BuildingTypeId::new("big_farm"),
            },
        ],
    );
    assert!(result.rejected.is_empty(), "{:?}", result.rejected);
    let effects: Vec<_> = result
        .accepted
        .iter()
        .map(|action| action.effect.clone())
        .collect();
    assert_eq!(
        effects,
        vec![
            ActionEffect::ProjectQueued {
                project_id: ProjectId(1)
            },
            ActionEffect::ProjectQueued {
                project_id: ProjectId(2)
            },
        ]
    );
    match &result.accepted[0].command {
        SettlementCommand::Construct {
            project_id,
            position,
            definition,
            cost,
        } => {
            assert_eq!(*project_id, ProjectId(1));
            assert_eq!(*position, at(1, 0));
            assert_eq!(definition.id(), "farm");
            assert_eq!(cost.needed_resources, HashMap::from([(Wood, 2)]));
            assert_eq!(cost.turn_cost, 3);
        }
        other => panic!("{other:?}"),
    }
    match &result.accepted[1].command {
        SettlementCommand::Upgrade {
            project_id,
            building_id,
            definition,
            cost,
        } => {
            assert_eq!(*project_id, ProjectId(2));
            assert_eq!(*building_id, FARM);
            assert_eq!(definition.id(), "big_farm");
            assert_eq!(cost.needed_resources, HashMap::from([(Wood, 5)]));
            assert_eq!(cost.turn_cost, 4);
        }
        other => panic!("{other:?}"),
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
    let result = validate(&world, vec![construct("farm", at(1, 0))]);
    assert!(matches!(
        result.accepted[0].command,
        SettlementCommand::Construct {
            project_id: ProjectId(8),
            ..
        }
    ));
}

#[test]
fn construction_sites_follow_placement_and_ownership_rules() {
    let mut world = world();
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    world
        .queue_construction(ME, ProjectId(3), at(0, 1), farm, ProjectCost::default())
        .unwrap();
    let result = validate(
        &world,
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
    assert!(result.accepted.is_empty());
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
    let world = world();
    let result = validate(
        &world,
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
    let result = validate(
        &world,
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
    for (action, expected) in result.accepted.iter().zip([4, 1]) {
        let SettlementCommand::SetBuildingStaff { load, .. } = action.command else {
            panic!("{:?}", action.command);
        };
        assert_eq!(Building::workers_at_load(4, load), expected);
    }
}

#[test]
fn staff_can_be_reduced_when_buildings_already_ask_for_too_many() {
    let mut world = world();
    let second = second_farm(&mut world);
    for building_id in [FARM, second] {
        world
            .execute_settlement_command(
                ME,
                &SettlementCommand::SetBuildingLoad {
                    building_id,
                    load: 1.0,
                },
            )
            .unwrap();
    }
    // 8 requested workers for a population of 5.
    let result = validate(&world, vec![workers(FARM, 3), workers(second, 4)]);
    assert!(result.rejected.is_empty(), "{:?}", result.rejected);
    let result = validate(&world, vec![workers(FARM, 0), workers(second, 4)]);
    assert!(result.rejected.is_empty(), "{:?}", result.rejected);
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
    let result = validate(
        &world,
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
    let result = validate(&world, vec![cancel(4), cancel(3)]);
    assert_eq!(
        errors(&result),
        vec![(
            0,
            ActionError::UnknownProject {
                project_id: ProjectId(4)
            }
        )]
    );
    assert!(matches!(
        result.accepted[0].command,
        SettlementCommand::CancelProject(ProjectId(3))
    ));
}

#[test]
fn storage_orders_use_the_engine_rule() {
    let world = world();
    let order = |order: Vec<ResourceType>| SettlementAction::SetStorageOrder { order };
    for (materials, error) in [
        (
            vec![Wood, Wood, Grain],
            StorageOrderError::DuplicateMaterial(Wood),
        ),
        (vec![Wood, Stone], StorageOrderError::MissingMaterial(Grain)),
        (
            vec![Labour, Wood, Stone, Grain],
            StorageOrderError::LabourIsNotMaterial,
        ),
    ] {
        let result = validate(&world, vec![order(materials)]);
        assert_eq!(
            errors(&result),
            vec![(0, ActionError::InvalidStorageOrder { error })]
        );
    }
    let result = validate(&world, vec![order(vec![Grain, Stone, Wood])]);
    assert!(result.rejected.is_empty());
}

#[test]
fn market_offers_are_checked_and_resolved_to_fresh_offers() {
    let world = world();
    for (offers, error) in [
        (
            vec![offer(Labour, OfferSide::Sell, 1, 1)],
            ActionError::LabourIsNotTradable,
        ),
        (
            vec![offer(Wood, OfferSide::Sell, 0, 1)],
            ActionError::ZeroUnits { resource: Wood },
        ),
        (
            vec![
                offer(Wood, OfferSide::Sell, 2, 1),
                offer(Wood, OfferSide::Buy, 2, 1),
            ],
            ActionError::DuplicateOffer { resource: Wood },
        ),
    ] {
        let result = validate(&world, vec![publish(offers)]);
        assert_eq!(errors(&result), vec![(0, error)]);
    }
    // Market offers are not limited by stock: each turn reserves what is available.
    let result = validate(
        &world,
        vec![publish(vec![
            offer(Wood, OfferSide::Sell, 500, 2),
            offer(Stone, OfferSide::Buy, 4, 3),
        ])],
    );
    let SettlementCommand::PublishMarketOffers(offers) = &result.accepted[0].command else {
        panic!("{:?}", result.accepted);
    };
    assert_eq!(
        offers,
        &HashMap::from([
            (Wood, TradeOffer::sell(500, 2)),
            (Stone, TradeOffer::buy(4, 3))
        ])
    );
    let result = validate(&world, vec![SettlementAction::ClearMarketOffers]);
    assert!(matches!(
        result.accepted[0].command,
        SettlementCommand::ClearMarketOffers
    ));
}

#[test]
fn caravans_need_a_receiver_and_enough_stock_and_gold_together() {
    let world = world();
    let result = validate(
        &world,
        vec![
            caravan(NEIGHBOUR, vec![]),
            caravan(ME, vec![offer(Wood, OfferSide::Sell, 1, 1)]),
            caravan(SettlementId(99), vec![offer(Wood, OfferSide::Sell, 1, 1)]),
            caravan(NEIGHBOUR, vec![offer(Labour, OfferSide::Sell, 1, 1)]),
            caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, 6, 1)]),
            caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, 6, 1)]),
            caravan(NEIGHBOUR, vec![offer(Grain, OfferSide::Buy, 4, 3)]),
            caravan(NEIGHBOUR, vec![offer(Stone, OfferSide::Buy, 5, 2)]),
            caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, 4, 1)]),
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
                    resource: Wood,
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
    world
        .execute_settlement_command(
            ME,
            &SettlementCommand::PublishMarketOffers(
                offers
                    .iter()
                    .map(|offer| (offer.resource, offer.to_offer()))
                    .collect(),
            ),
        )
        .unwrap();
    world
}

#[test]
fn caravans_get_only_what_the_published_market_leaves() {
    // Stock: Wood 10, gold 20. The market reserves 7 Wood and 4 × 3 = 12 gold first.
    let world = world_with_market(vec![
        offer(Wood, OfferSide::Sell, 7, 1),
        offer(Stone, OfferSide::Buy, 4, 3),
    ]);
    let result = validate(
        &world,
        vec![
            caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, 4, 1)]),
            caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, 3, 1)]),
            caravan(NEIGHBOUR, vec![offer(Grain, OfferSide::Buy, 3, 3)]),
            caravan(NEIGHBOUR, vec![offer(Grain, OfferSide::Buy, 4, 2)]),
        ],
    );
    assert_eq!(
        errors(&result),
        vec![
            (
                0,
                ActionError::InsufficientStock {
                    resource: Wood,
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
    let world = world_with_market(vec![offer(Wood, OfferSide::Sell, 10, 1)]);
    let caravan_wood = |units| caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, units, 1)]);
    // Clearing or shrinking the market first frees its stock for caravans.
    let result = validate(
        &world,
        vec![SettlementAction::ClearMarketOffers, caravan_wood(10)],
    );
    assert!(result.rejected.is_empty(), "{:?}", result.rejected);
    let result = validate(
        &world,
        vec![
            publish(vec![offer(Wood, OfferSide::Sell, 6, 1)]),
            caravan_wood(4),
            caravan_wood(1),
        ],
    );
    assert_eq!(
        errors(&result),
        vec![(
            2,
            ActionError::InsufficientStock {
                resource: Wood,
                requested: 1,
                available: 0
            }
        )]
    );
}

#[test]
fn a_later_market_action_cannot_take_what_caravans_were_promised() {
    let world = world();
    let result = validate(
        &world,
        vec![
            caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, 6, 1)]),
            caravan(NEIGHBOUR, vec![offer(Grain, OfferSide::Buy, 5, 3)]),
            publish(vec![offer(Wood, OfferSide::Sell, 5, 1)]),
            publish(vec![offer(Stone, OfferSide::Buy, 2, 3)]),
            publish(vec![
                offer(Wood, OfferSide::Sell, 4, 1),
                offer(Stone, OfferSide::Buy, 1, 5),
            ]),
        ],
    );
    assert_eq!(
        errors(&result),
        vec![
            (
                2,
                ActionError::InsufficientStock {
                    resource: Wood,
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
    let result = validate(
        &world,
        vec![publish(vec![offer(Wood, OfferSide::Sell, 999, 1)])],
    );
    assert!(result.rejected.is_empty());
}

#[test]
fn caravans_queued_earlier_in_the_turn_count_too() {
    let mut world = world();
    let earlier = SettlementDecision::new(
        ME,
        vec![caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, 8, 1)])],
    );
    assert!(world.apply_decision(&earlier).unwrap().results[0].is_applied());
    let result = validate(
        &world,
        vec![caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, 3, 1)])],
    );
    assert_eq!(
        errors(&result),
        vec![(
            0,
            ActionError::InsufficientStock {
                resource: Wood,
                requested: 3,
                available: 2
            }
        )]
    );
}

#[test]
fn caravans_must_carry_goods_or_gold() {
    let world = world();
    let result = validate(
        &world,
        vec![
            caravan(NEIGHBOUR, vec![offer(Grain, OfferSide::Buy, 5, 0)]),
            caravan(NEIGHBOUR, vec![offer(Grain, OfferSide::Buy, 5, 1)]),
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
        order: vec![Grain, Stone, Wood],
    };
    let result = validate(
        &world,
        vec![
            construct("farm", at(1, 0)),
            construct("big_farm", at(1, 0)),
            workers(FARM, 1),
            workers(FARM, 2),
            publish(vec![offer(Wood, OfferSide::Sell, 1, 1)]),
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
fn rejected_actions_claim_nothing() {
    let world = world();
    let result = validate(
        &world,
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
    assert!(matches!(
        result.accepted[0].command,
        SettlementCommand::Construct {
            project_id: ProjectId(1),
            ..
        }
    ));
}

#[test]
fn every_action_is_either_accepted_or_rejected_in_order_and_deterministically() {
    let world = world();
    let actions = vec![
        workers(FARM, 2),
        construct("farm", at(3, 0)),
        caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, 3, 1)]),
        SettlementAction::CancelProject {
            project_id: ProjectId(1),
        },
        SettlementAction::ClearMarketOffers,
    ];
    let result = validate(&world, actions.clone());
    assert_eq!(accepted_indices(&result), vec![0, 2, 4]);
    assert_eq!(
        result.rejected,
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
    let again = validate(&world, actions);
    assert_eq!(format!("{again:?}"), format!("{result:?}"));
}

#[test]
fn validation_leaves_the_world_unchanged() {
    let world = world();
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
    let result = validate(
        &world,
        vec![
            construct("farm", at(1, 0)),
            workers(FARM, 4),
            SettlementAction::SetStorageOrder {
                order: vec![Grain, Stone, Wood],
            },
            publish(vec![offer(Wood, OfferSide::Sell, 1, 1)]),
            caravan(NEIGHBOUR, vec![offer(Wood, OfferSide::Sell, 1, 1)]),
        ],
    );
    assert_eq!(result.accepted.len(), 5);
    assert_eq!(snapshot(&world), before);
    assert_eq!(
        world.tile_at(at(1, 0)).unwrap().occupancy(),
        TileOccupancy::Empty
    );
}

#[test]
fn a_decision_of_an_unknown_settlement_is_rejected_as_a_whole() {
    let world = world();
    let decision = SettlementDecision::new(SettlementId(99), vec![workers(FARM, 1)]);
    assert!(matches!(
        world.validate_decision(&decision),
        Err(WorldError::MissingSettlement(SettlementId(99)))
    ));
}

#[test]
fn rejections_serialize_for_agents() {
    let world = world();
    let result = validate(&world, vec![construct("farm", at(0, 0)), workers(FARM, 5)]);
    assert_eq!(
        serde_json::to_value(&result.rejected).unwrap(),
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
            error: StorageOrderError::DuplicateMaterial(Wood),
        },
    };
    let json = serde_json::to_string(&storage).unwrap();
    assert_eq!(
        serde_json::from_str::<RejectedAction>(&json).unwrap(),
        storage
    );
}
