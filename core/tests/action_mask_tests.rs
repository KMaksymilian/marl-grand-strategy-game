//! Action masks: agreement with the validator and changes with resources,
//! workforce and construction.
use simulation_core::buildings::Building;
use simulation_core::buildings::catalog::BuildingCatalog;
use simulation_core::buildings::definition::{BuildingBehavior, BuildingDefinition};
use simulation_core::economy::ResourceType::{self, Grain, Labour, Stone, Wood};
use simulation_core::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use simulation_core::settlement::Settlement;
use simulation_core::settlement::action_mask::{
    ActionMask, ConstructOption, UpgradeOption, WorkerRange,
};
use simulation_core::settlement::command::SettlementCommand;
use simulation_core::settlement::decision::{ActionKind, OfferSide, OfferSpec};
use simulation_core::settlement::projects::ProjectCost;
use simulation_core::simulation::{SettlementAction, SettlementDecision, Simulation};
use simulation_core::world::pathfinding::path_finder::PathFinder;
use simulation_core::world::{Map, Position, Province, World, terrain::area::Area};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

const ME: SettlementId = SettlementId(1);
const NEIGHBOUR: SettlementId = SettlementId(2);
const FARM: BuildingId = BuildingId(1);

fn at(x: i32, y: i32) -> Position {
    Position { x, y }
}

fn kind(id: &str) -> BuildingTypeId {
    BuildingTypeId::new(id)
}

fn definition(id: &str, max_workers: u32, wood: u32, turns: u32) -> BuildingDefinition {
    BuildingDefinition::new(
        id,
        id,
        1,
        max_workers,
        vec![BuildingBehavior::Production {
            inputs: vec![],
            outputs: vec![(Grain, 1)],
        }],
        ProjectCost {
            needed_resources: HashMap::from([(Wood, wood)]),
            turn_cost: turns,
        },
    )
    .unwrap()
}

/// Provinces `10 10 20 20 30 / 10 ~10 20 20 30` (`~` is ocean). Settlement 1
/// (province 10): population 3, Wood 10, gold 15, a farm at (0,0). Settlement 2
/// (province 20) has a building and a project of its own.
fn world() -> World {
    let mut terrain = vec![Area::new(0.5, 0.5); 10];
    terrain[6] = Area::new(0.0, 0.5);
    let mut world = World::new(
        Map::new(5, 2, terrain, vec![10, 10, 20, 20, 30, 10, 10, 20, 20, 30]),
        vec![Province::new(10), Province::new(20), Province::new(30)],
    );
    let mut catalog = BuildingCatalog::default();
    catalog.register(definition("farm", 2, 2, 2));
    catalog.register(definition("mill", 3, 0, 3));
    world.set_building_catalog(catalog);
    let mut me = Settlement::new(ME, 100, 3, Arc::default());
    me.deposit(Wood, 10);
    me.deposit_gold(15);
    world.add_settlement(10, me).unwrap();
    world
        .add_settlement(20, Settlement::new(NEIGHBOUR, 100, 2, Arc::default()))
        .unwrap();
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    let mut finder = PathFinder::new(5, 2);
    world
        .add_building(ME, Building::new(FARM, at(0, 0), farm.clone()), &mut finder)
        .unwrap();
    world
        .add_building(
            NEIGHBOUR,
            Building::new(BuildingId(5), at(2, 0), farm.clone()),
            &mut finder,
        )
        .unwrap();
    world
        .queue_construction(
            NEIGHBOUR,
            ProjectId(3),
            at(3, 0),
            farm,
            ProjectCost::default(),
        )
        .unwrap();
    world
}

fn mask(simulation: &Simulation) -> ActionMask {
    simulation.observe(ME).unwrap().action_mask
}

fn step_with(simulation: &mut Simulation, actions: Vec<SettlementAction>) {
    simulation
        .queue_decision(SettlementDecision::new(ME, actions))
        .unwrap();
    let report = simulation.step().unwrap();
    assert!(
        report.decisions[0]
            .results
            .iter()
            .all(|result| result.is_applied()),
        "{:?}",
        report.decisions
    );
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

/// Valid and invalid actions of every kind, including off-map positions, foreign
/// and missing IDs, out-of-range staff and offers at and beyond the limits.
fn candidates(simulation: &Simulation) -> Vec<SettlementAction> {
    let me = simulation.settlement(ME).unwrap();
    let types = ["farm", "mill", "castle"];
    let mut actions = Vec::new();
    for x in -1..=5 {
        for y in -1..=2 {
            for building_type in types {
                actions.push(SettlementAction::Construct {
                    building_type: kind(building_type),
                    position: at(x, y),
                });
            }
        }
    }
    for id in 0..=6 {
        for building_type in types {
            actions.push(SettlementAction::Upgrade {
                building_id: BuildingId(id),
                building_type: kind(building_type),
            });
        }
        actions.push(SettlementAction::CancelProject {
            project_id: ProjectId(id),
        });
        for staff in 0..=5 {
            actions.push(workers(BuildingId(id), staff));
        }
    }
    actions.push(SettlementAction::SetStorageOrder {
        order: vec![Grain, Stone, Wood],
    });
    actions.push(SettlementAction::SetStorageOrder {
        order: vec![Grain, Grain, Wood],
    });
    actions.push(SettlementAction::PublishMarketOffers {
        offers: vec![
            offer(Wood, OfferSide::Sell, 50, 1),
            offer(Stone, OfferSide::Buy, 9, 9),
        ],
    });
    actions.push(SettlementAction::PublishMarketOffers {
        offers: vec![offer(Labour, OfferSide::Sell, 1, 1)],
    });
    actions.push(SettlementAction::ClearMarketOffers);
    for receiver in 0..=3 {
        actions.push(SettlementAction::OrderCaravan {
            receiver_id: SettlementId(receiver),
            offers: vec![offer(Grain, OfferSide::Buy, 1, 0)],
        });
    }
    for resource in ResourceType::MATERIALS {
        let stock = me.inventory().amount(*resource);
        for units in [stock, stock + 1] {
            actions.push(SettlementAction::OrderCaravan {
                receiver_id: NEIGHBOUR,
                offers: vec![offer(*resource, OfferSide::Sell, units, 1)],
            });
        }
    }
    for price in [me.gold(), me.gold() + 1] {
        actions.push(SettlementAction::OrderCaravan {
            receiver_id: NEIGHBOUR,
            offers: vec![offer(Wood, OfferSide::Buy, 1, price)],
        });
    }
    actions.push(SettlementAction::OrderCaravan {
        receiver_id: NEIGHBOUR,
        offers: vec![],
    });
    actions
}

fn offers_fit(mask: &ActionMask, offers: &[OfferSpec]) -> bool {
    let mut seen = HashSet::new();
    offers.iter().all(|offer| {
        mask.tradable_resources.contains(&offer.resource)
            && offer.units > 0
            && seen.insert(offer.resource)
    })
}

/// How an agent reads the mask for a single action.
fn mask_allows(mask: &ActionMask, action: &SettlementAction) -> bool {
    match action {
        SettlementAction::Construct {
            building_type,
            position,
        } => mask.construct.iter().any(|option| {
            option.position == *position && option.building_types.contains(building_type)
        }),
        SettlementAction::Upgrade {
            building_id,
            building_type,
        } => mask.upgrade.iter().any(|option| {
            option.building_id == *building_id && option.building_types.contains(building_type)
        }),
        SettlementAction::CancelProject { project_id } => mask.cancel_project.contains(project_id),
        SettlementAction::SetBuildingWorkers {
            building_id,
            workers,
        } => mask
            .set_building_workers
            .iter()
            .any(|range| range.building_id == *building_id && *workers <= range.max_workers),
        SettlementAction::SetStorageOrder { order } => {
            let mut sorted = order.clone();
            sorted.sort();
            sorted.dedup();
            sorted.len() == order.len() && {
                let mut materials = mask.storage_materials.clone();
                materials.sort();
                materials == sorted
            }
        }
        SettlementAction::PublishMarketOffers { offers } => offers_fit(mask, offers),
        SettlementAction::ClearMarketOffers => mask.is_available(ActionKind::ClearMarketOffers),
        SettlementAction::OrderCaravan {
            receiver_id,
            offers,
        } => {
            let sells_fit = offers.iter().all(|offer| {
                offer.side == OfferSide::Buy
                    || offer.units
                        <= mask
                            .caravan_sell_limits
                            .get(&offer.resource)
                            .copied()
                            .unwrap_or(0)
            });
            let budget: u64 = offers
                .iter()
                .filter(|offer| offer.side == OfferSide::Buy)
                .map(|offer| u64::from(offer.units) * u64::from(offer.price_per_unit))
                .sum();
            mask.caravan_receivers.contains(receiver_id)
                && !offers.is_empty()
                && offers_fit(mask, offers)
                && sells_fit
                && budget <= u64::from(mask.caravan_gold_limit)
        }
    }
}

/// For every candidate, the mask and the validator give the same answer, and a
/// kind is available exactly when some candidate of that kind is valid.
fn assert_mask_matches_validator(simulation: &Simulation) {
    let mask = mask(simulation);
    let mut valid_kinds = HashSet::new();
    for action in candidates(simulation) {
        let result = simulation
            .world()
            .validate_decision(&SettlementDecision::new(ME, vec![action.clone()]))
            .unwrap();
        let valid = result.rejected.is_empty();
        assert_eq!(
            mask_allows(&mask, &action),
            valid,
            "{action:?}: {:?}",
            result.rejected
        );
        if valid {
            valid_kinds.insert(action.kind());
        }
    }
    for kind in ActionKind::ALL {
        assert_eq!(
            mask.is_available(kind),
            valid_kinds.contains(&kind),
            "{kind:?}"
        );
    }
}

#[test]
fn masks_agree_with_the_validator_while_the_state_changes() {
    let mut simulation = Simulation::new(world()).unwrap();
    assert_mask_matches_validator(&simulation);
    // A construction site, staffed farm and spent wood.
    step_with(
        &mut simulation,
        vec![
            SettlementAction::Construct {
                building_type: kind("farm"),
                position: at(0, 1),
            },
            workers(FARM, 2),
        ],
    );
    assert_mask_matches_validator(&simulation);
    // The farm is finished: a second building, partly used workforce.
    step_with(&mut simulation, vec![]);
    assert_mask_matches_validator(&simulation);
    // A running upgrade and a market with reserved stock.
    step_with(
        &mut simulation,
        vec![
            SettlementAction::Upgrade {
                building_id: FARM,
                building_type: kind("mill"),
            },
            SettlementAction::PublishMarketOffers {
                offers: vec![offer(Wood, OfferSide::Sell, 3, 1)],
            },
        ],
    );
    assert_mask_matches_validator(&simulation);
}

#[test]
fn masks_agree_with_the_validator_when_staff_exceeds_the_population() {
    let mut world = world();
    let farm = world.building_catalog().get_blueprint("farm").unwrap();
    world
        .add_building(
            ME,
            Building::new(BuildingId(2), at(0, 1), farm),
            &mut PathFinder::new(5, 2),
        )
        .unwrap();
    for building_id in [FARM, BuildingId(2)] {
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
    // 4 requested workers for a population of 3: only keeping or lowering staff is valid.
    let simulation = Simulation::new(world).unwrap();
    assert_eq!(
        mask(&simulation).set_building_workers,
        vec![
            WorkerRange {
                building_id: FARM,
                max_workers: 2
            },
            WorkerRange {
                building_id: BuildingId(2),
                max_workers: 2
            },
        ]
    );
    assert_mask_matches_validator(&simulation);
}

#[test]
fn construction_changes_sites_projects_and_targets() {
    let mut simulation = Simulation::new(world()).unwrap();
    let all_types = vec![kind("farm"), kind("mill")];
    let initial = mask(&simulation);
    // (0,0) holds the farm and (1,1) is ocean.
    assert_eq!(
        initial.construct,
        vec![
            ConstructOption {
                position: at(1, 0),
                building_types: all_types.clone()
            },
            ConstructOption {
                position: at(0, 1),
                building_types: all_types.clone()
            },
        ]
    );
    assert!(initial.cancel_project.is_empty());
    assert!(!initial.is_available(ActionKind::CancelProject));
    assert_eq!(
        initial.upgrade,
        vec![UpgradeOption {
            building_id: FARM,
            building_types: vec![kind("mill")]
        }]
    );

    step_with(
        &mut simulation,
        vec![SettlementAction::Construct {
            building_type: kind("farm"),
            position: at(0, 1),
        }],
    );
    let building = mask(&simulation);
    assert_eq!(
        building
            .construct
            .iter()
            .map(|option| option.position)
            .collect::<Vec<_>>(),
        vec![at(1, 0)]
    );
    assert_eq!(building.cancel_project, vec![ProjectId(1)]);
    assert!(building.is_available(ActionKind::CancelProject));

    step_with(&mut simulation, vec![]);
    let finished = mask(&simulation);
    assert!(finished.cancel_project.is_empty());
    let new_building = simulation
        .settlement(ME)
        .unwrap()
        .buildings()
        .find(|building| building.position() == at(0, 1))
        .unwrap()
        .instance_id();
    assert!(
        finished
            .set_building_workers
            .iter()
            .any(|range| range.building_id == new_building)
    );
    assert!(
        finished
            .upgrade
            .iter()
            .any(|option| option.building_id == new_building)
    );

    step_with(
        &mut simulation,
        vec![SettlementAction::Upgrade {
            building_id: new_building,
            building_type: kind("mill"),
        }],
    );
    let upgrading = mask(&simulation);
    assert!(
        upgrading
            .upgrade
            .iter()
            .all(|option| option.building_id != new_building)
    );
    assert_eq!(upgrading.cancel_project, vec![ProjectId(2)]);
}

#[test]
fn canceling_a_construction_frees_its_site_again() {
    let mut simulation = Simulation::new(world()).unwrap();
    step_with(
        &mut simulation,
        vec![SettlementAction::Construct {
            building_type: kind("mill"),
            position: at(1, 0),
        }],
    );
    assert!(
        mask(&simulation)
            .construct
            .iter()
            .all(|option| option.position != at(1, 0))
    );
    step_with(
        &mut simulation,
        vec![SettlementAction::CancelProject {
            project_id: ProjectId(1),
        }],
    );
    assert!(
        mask(&simulation)
            .construct
            .iter()
            .any(|option| option.position == at(1, 0))
    );
}

#[test]
fn worker_ranges_follow_the_free_population() {
    let mut world = world();
    let mill = world.building_catalog().get_blueprint("mill").unwrap();
    world
        .add_building(
            ME,
            Building::new(BuildingId(2), at(0, 1), mill),
            &mut PathFinder::new(5, 2),
        )
        .unwrap();
    let mut simulation = Simulation::new(world).unwrap();
    // Population 3, nobody employed: each building up to its own capacity.
    assert_eq!(
        mask(&simulation).set_building_workers,
        vec![
            WorkerRange {
                building_id: FARM,
                max_workers: 2
            },
            WorkerRange {
                building_id: BuildingId(2),
                max_workers: 3
            },
        ]
    );
    step_with(&mut simulation, vec![workers(FARM, 2)]);
    // Two workers are taken: the mill can have one, the farm keeps its two.
    assert_eq!(
        mask(&simulation).set_building_workers,
        vec![
            WorkerRange {
                building_id: FARM,
                max_workers: 2
            },
            WorkerRange {
                building_id: BuildingId(2),
                max_workers: 1
            },
        ]
    );
}

#[test]
fn caravan_limits_follow_stock_and_gold() {
    let mut simulation = Simulation::new(world()).unwrap();
    let initial = mask(&simulation);
    assert_eq!(
        initial.caravan_sell_limits,
        BTreeMap::from([(Wood, 10), (Stone, 0), (Grain, 0)])
    );
    assert_eq!(initial.caravan_gold_limit, 15);
    assert_eq!(initial.caravan_receivers, vec![NEIGHBOUR]);
    // Construction pays 2 Wood; the staffed farm makes Grain; the caravan buys with gold.
    step_with(
        &mut simulation,
        vec![
            SettlementAction::Construct {
                building_type: kind("farm"),
                position: at(1, 0),
            },
            workers(FARM, 2),
            SettlementAction::OrderCaravan {
                receiver_id: NEIGHBOUR,
                offers: vec![offer(Stone, OfferSide::Buy, 2, 5)],
            },
        ],
    );
    let later = mask(&simulation);
    assert_eq!(
        later.caravan_sell_limits,
        BTreeMap::from([(Wood, 8), (Stone, 0), (Grain, 1)])
    );
    assert_eq!(later.caravan_gold_limit, 5);
    assert_mask_matches_validator(&simulation);
}

#[test]
fn kinds_without_any_valid_target_are_unavailable() {
    // A lone settlement in an ocean province with nothing built.
    let mut world = World::new(
        Map::new(1, 1, vec![Area::new(0.0, 0.5)], vec![10]),
        vec![Province::new(10)],
    );
    let mut catalog = BuildingCatalog::default();
    catalog.register(definition("farm", 2, 2, 2));
    world.set_building_catalog(catalog);
    world
        .add_settlement(10, Settlement::new(ME, 100, 3, Arc::default()))
        .unwrap();
    let simulation = Simulation::new(world).unwrap();
    let mask = mask(&simulation);
    let available: Vec<_> = ActionKind::ALL
        .into_iter()
        .filter(|kind| mask.is_available(*kind))
        .collect();
    assert_eq!(
        available,
        vec![
            ActionKind::SetStorageOrder,
            ActionKind::PublishMarketOffers,
            ActionKind::ClearMarketOffers
        ]
    );
    assert!(mask.construct.is_empty() && mask.upgrade.is_empty());
    assert!(mask.set_building_workers.is_empty() && mask.caravan_receivers.is_empty());
}

#[test]
fn the_mask_is_part_of_the_serialized_observation() {
    let simulation = Simulation::new(world()).unwrap();
    let observation = simulation.observe(ME).unwrap();
    let json: serde_json::Value = serde_json::from_str(&observation.to_json()).unwrap();
    assert_eq!(
        json["action_mask"]["available"],
        serde_json::json!({
            "construct": true, "upgrade": true, "cancel_project": false,
            "set_building_workers": true, "set_storage_order": true,
            "publish_market_offers": true, "clear_market_offers": true,
            "order_caravan": true
        })
    );
    assert_eq!(
        json["action_mask"]["set_building_workers"],
        serde_json::json!([{"building_id": 1, "max_workers": 2}])
    );
    // The neighbour's own mask lists its own targets, not ours.
    let neighbour = simulation.observe(NEIGHBOUR).unwrap().action_mask;
    assert_eq!(neighbour.cancel_project, vec![ProjectId(3)]);
    assert_eq!(neighbour.caravan_receivers, vec![ME]);
    assert!(
        neighbour
            .set_building_workers
            .iter()
            .all(|range| range.building_id == BuildingId(5))
    );
}
