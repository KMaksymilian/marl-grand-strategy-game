//! Read-only validation of agent decisions against the authoritative world.
//!
//! Policy: actions are checked in submission order and each one is accepted or
//! rejected on its own. Accepted actions are resolved to `SettlementCommand`s with
//! catalog definitions and costs, simulation-assigned project IDs and exact building
//! loads. A later action that targets the same object as an earlier accepted action
//! is rejected as a conflict. Validation never changes the world.
use super::validation::validate_input;
use crate::application::commands::settlement::contract::{
    ActionEffect, ActionError, OfferSpec, RejectedAction, SettlementAction, SettlementDecision,
};
use crate::application::commands::settlement::prepared::SettlementCommand;
use crate::domain::buildings::Building;
use crate::domain::buildings::definition::BuildingDefinition;
use crate::domain::economy::ResourceType;
use crate::domain::economy::storage_order::StorageOrder;
use crate::domain::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use crate::domain::settlement::Settlement;
use crate::domain::settlement::projects::{ProjectError, ProjectResult};
use crate::domain::spatial::{Position, TerritoryError};
use crate::domain::trade::offer::TradeOffer;
use crate::domain::world::{World, WorldError};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// An accepted action, resolved to the command that executes it and the effect
/// reported once that command has run.
#[derive(Debug, Clone)]
pub struct AcceptedAction {
    /// Position in `SettlementDecision::actions`.
    pub index: usize,
    pub command: SettlementCommand,
    pub effect: ActionEffect,
}

/// Result of validating one decision. Both lists keep submission order, and
/// every action index appears in exactly one of them.
#[derive(Debug, Clone)]
pub struct ValidatedDecision {
    pub settlement_id: SettlementId,
    pub accepted: Vec<AcceptedAction>,
    pub rejected: Vec<RejectedAction>,
}

/// Objects that at most one action of a decision may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Target {
    Tile(Position),
    Upgrade(BuildingId),
    Project(ProjectId),
    Workers(BuildingId),
    StorageOrder,
    Market,
}

/// What a checked action would use; applied to the batch only when it is accepted.
struct Checked {
    command: SettlementCommand,
    effect: ActionEffect,
    target: Option<Target>,
    new_project: bool,
    staff: Option<(BuildingId, u32)>,
    caravan_goods: BTreeMap<ResourceType, u64>,
    caravan_gold: u64,
    /// New market reservation (goods, gold) after a publish or clear action.
    market: Option<(BTreeMap<ResourceType, u64>, u64)>,
}

impl Checked {
    fn new(command: SettlementCommand, effect: ActionEffect) -> Self {
        Self {
            command,
            effect,
            target: None,
            new_project: false,
            staff: None,
            caravan_goods: BTreeMap::new(),
            caravan_gold: 0,
            market: None,
        }
    }

    fn target(mut self, target: Target) -> Self {
        self.target = Some(target);
        self
    }
}

/// Settlement state as changed by the decision's earlier accepted actions.
///
/// Trade is tracked in funding order: at the start of the turn the market is
/// funded first, then caravan orders in queue order, both from the stock and gold
/// the settlement holds now (nothing changes them between decisions and funding).
pub(in crate::application) struct Batch {
    claimed: HashMap<Target, usize>,
    next_project: u64,
    /// Staff requested by each building before this decision.
    staff_before: HashMap<BuildingId, u32>,
    total_staff: u64,
    /// What the market will reserve: published offers, or this decision's new ones.
    market_goods: BTreeMap<ResourceType, u64>,
    market_gold: u64,
    /// Committed to caravans: orders queued earlier this turn plus this decision's.
    caravan_goods: BTreeMap<ResourceType, u64>,
    caravan_gold: u64,
}

impl Batch {
    pub(in crate::application) fn new(settlement: &Settlement) -> Self {
        let staff_before: HashMap<_, _> = settlement
            .buildings()
            .map(|building| (building.instance_id(), building.requested_workers()))
            .collect();
        let (market_goods, market_gold) = offer_demand(settlement.market_offers());
        let mut caravan_goods = BTreeMap::new();
        let mut caravan_gold = 0;
        for order in settlement.caravan_orders() {
            let (goods, gold) = offer_demand(&order.offers);
            for (resource, amount) in goods {
                *caravan_goods.entry(resource).or_default() += amount;
            }
            caravan_gold += gold;
        }
        Self {
            claimed: HashMap::new(),
            next_project: settlement
                .next_project_id()
                .map_or(u64::from(u32::MAX) + 1, |id| u64::from(id.0)),
            total_staff: staff_before.values().map(|&staff| u64::from(staff)).sum(),
            staff_before,
            market_goods,
            market_gold,
            caravan_goods,
            caravan_gold,
        }
    }

    /// Units of `resource` a new caravan can still be funded with: current stock
    /// minus what the market reserves first and what earlier caravans take.
    pub(in crate::application) fn caravan_room(
        &self,
        settlement: &Settlement,
        resource: ResourceType,
    ) -> u64 {
        let stock = u64::from(settlement.inventory().amount(resource));
        let market = self
            .market_goods
            .get(&resource)
            .copied()
            .unwrap_or(0)
            .min(stock);
        (stock - market).saturating_sub(self.caravan_goods.get(&resource).copied().unwrap_or(0))
    }

    /// Gold a new caravan can still commit to purchases, after the market and
    /// earlier caravans.
    pub(in crate::application) fn caravan_gold_room(&self, settlement: &Settlement) -> u64 {
        let gold = u64::from(settlement.gold());
        (gold - self.market_gold.min(gold)).saturating_sub(self.caravan_gold)
    }

    /// A new market reservation must leave the goods and gold already committed to
    /// caravans, because the market is funded before them.
    fn check_market(
        &self,
        settlement: &Settlement,
        goods: &BTreeMap<ResourceType, u64>,
        gold: u64,
    ) -> Result<(), ActionError> {
        for (&resource, &committed) in &self.caravan_goods {
            let stock = u64::from(settlement.inventory().amount(resource));
            let requested = goods.get(&resource).copied().unwrap_or(0);
            if committed > 0 && requested + committed > stock {
                return Err(ActionError::InsufficientStock {
                    resource,
                    requested,
                    available: room_u32(stock.saturating_sub(committed)),
                });
            }
        }
        let treasury = u64::from(settlement.gold());
        if self.caravan_gold > 0 && gold + self.caravan_gold > treasury {
            return Err(ActionError::InsufficientGold {
                requested: gold,
                available: room_u32(treasury.saturating_sub(self.caravan_gold)),
            });
        }
        Ok(())
    }

    fn ensure_free(&self, target: Target) -> Result<(), ActionError> {
        match self.claimed.get(&target) {
            Some(&with_index) => Err(ActionError::Conflict { with_index }),
            None => Ok(()),
        }
    }

    /// Highest staff the building may request. Raising staff must keep all
    /// buildings within the population; keeping or lowering staff is always allowed.
    pub(in crate::application) fn worker_limit(
        &self,
        settlement: &Settlement,
        building_id: BuildingId,
        max_workers: u32,
    ) -> u32 {
        let before = self.staff_before[&building_id];
        let others = self.total_staff - u64::from(before);
        let free = u64::from(settlement.population()).saturating_sub(others);
        u32::try_from(free.max(u64::from(before)))
            .unwrap_or(u32::MAX)
            .min(max_workers)
    }

    fn project_id(&self) -> Result<ProjectId, ActionError> {
        u32::try_from(self.next_project)
            .map(ProjectId)
            .map_err(|_| ActionError::ProjectIdsExhausted)
    }

    fn accept(&mut self, index: usize, checked: &Checked) {
        if let Some(target) = checked.target {
            self.claimed.insert(target, index);
        }
        if checked.new_project {
            self.next_project += 1;
        }
        if let Some((building_id, workers)) = checked.staff {
            self.total_staff =
                self.total_staff - u64::from(self.staff_before[&building_id]) + u64::from(workers);
        }
        for (&resource, &amount) in &checked.caravan_goods {
            *self.caravan_goods.entry(resource).or_default() += amount;
        }
        self.caravan_gold += checked.caravan_gold;
        if let Some((goods, gold)) = &checked.market {
            self.market_goods = goods.clone();
            self.market_gold = *gold;
        }
    }
}

/// Goods (sell offers) and gold (buy budgets) that offers reserve when funded.
fn offer_demand<'a>(
    offers: impl IntoIterator<Item = (&'a ResourceType, &'a TradeOffer)>,
) -> (BTreeMap<ResourceType, u64>, u64) {
    let mut goods = BTreeMap::new();
    let mut gold = 0;
    for (&resource, offer) in offers {
        match offer {
            TradeOffer::Sell(sell) => {
                *goods.entry(resource).or_default() += u64::from(sell.get_amount_to_sell())
            }
            TradeOffer::Buy(buy) => {
                gold += u64::from(buy.get_remaining_amount_to_buy())
                    * u64::from(buy.get_maximum_price_per_unit())
            }
        }
    }
    (goods, gold)
}

/// Stock and gold rooms never exceed the `u32` amounts they are derived from.
fn room_u32(amount: u64) -> u32 {
    u32::try_from(amount).unwrap_or(u32::MAX)
}

pub(in crate::application) struct DecisionPreparation<'a> {
    world: &'a World,
}

impl<'a> DecisionPreparation<'a> {
    pub(in crate::application) fn new(world: &'a World) -> Self {
        Self { world }
    }

    /// Checks every action without changing the world. Fails as a whole only when
    /// the deciding settlement does not exist.
    pub(super) fn validate_decision(
        &self,
        decision: &SettlementDecision,
    ) -> Result<ValidatedDecision, WorldError> {
        let settlement = self
            .world
            .settlement(decision.settlement_id)
            .ok_or(WorldError::MissingSettlement(decision.settlement_id))?;
        let mut batch = Batch::new(settlement);
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        for (index, action) in decision.actions.iter().enumerate() {
            match self.prepare_action(settlement, action, &batch) {
                Ok(checked) => {
                    batch.accept(index, &checked);
                    accepted.push(AcceptedAction {
                        index,
                        command: checked.command,
                        effect: checked.effect,
                    });
                }
                Err(error) => rejected.push(RejectedAction {
                    index,
                    kind: action.kind(),
                    error,
                }),
            }
        }
        Ok(ValidatedDecision {
            settlement_id: decision.settlement_id,
            accepted,
            rejected,
        })
    }

    /// The validator's verdict on `action` as the first action of a decision.
    pub(in crate::application) fn check_single_action(
        &self,
        settlement: &Settlement,
        action: &SettlementAction,
        batch: &Batch,
    ) -> Result<(), ActionError> {
        self.prepare_action(settlement, action, batch).map(|_| ())
    }

    fn prepare_action(
        &self,
        settlement: &Settlement,
        action: &SettlementAction,
        batch: &Batch,
    ) -> Result<Checked, ActionError> {
        validate_input(action)?;
        self.check_action(settlement, action, batch)
    }

    fn check_action(
        &self,
        settlement: &Settlement,
        action: &SettlementAction,
        batch: &Batch,
    ) -> Result<Checked, ActionError> {
        match action {
            SettlementAction::Construct {
                building_type,
                position,
            } => {
                let definition = self.catalog_definition(building_type)?;
                self.check_construction_site(settlement.id(), *position)?;
                batch.ensure_free(Target::Tile(*position))?;
                let project_id = batch.project_id()?;
                let mut checked = Checked::new(
                    SettlementCommand::Construct {
                        project_id,
                        position: *position,
                        cost: definition.construction_cost().clone(),
                        definition,
                    },
                    ActionEffect::ProjectQueued { project_id },
                )
                .target(Target::Tile(*position));
                checked.new_project = true;
                Ok(checked)
            }
            SettlementAction::Upgrade {
                building_id,
                building_type,
            } => {
                let building = own_building(settlement, *building_id)?;
                let definition = self.catalog_definition(building_type)?;
                if building.definition().id() == definition.id() {
                    return Err(ActionError::SameBuildingType {
                        building_id: *building_id,
                    });
                }
                let upgrading = settlement.active_projects().iter().any(|project| {
                    matches!(project.result(), ProjectResult::UpgradeBuilding { target_instance_id, .. }
                        if target_instance_id == building_id)
                });
                if upgrading {
                    return Err(ActionError::BuildingAlreadyUpgrading {
                        building_id: *building_id,
                    });
                }
                self.world
                    .validate_settlement_position(settlement.id(), building.position())
                    .map_err(|_| ActionError::OutsideOwnProvince {
                        position: building.position(),
                    })?;
                batch.ensure_free(Target::Upgrade(*building_id))?;
                let project_id = batch.project_id()?;
                let mut checked = Checked::new(
                    SettlementCommand::Upgrade {
                        project_id,
                        building_id: *building_id,
                        cost: definition.construction_cost().clone(),
                        definition,
                    },
                    ActionEffect::ProjectQueued { project_id },
                )
                .target(Target::Upgrade(*building_id));
                checked.new_project = true;
                Ok(checked)
            }
            SettlementAction::CancelProject { project_id } => {
                if !settlement
                    .active_projects()
                    .iter()
                    .any(|project| project.id() == *project_id)
                {
                    return Err(ActionError::UnknownProject {
                        project_id: *project_id,
                    });
                }
                batch.ensure_free(Target::Project(*project_id))?;
                Ok(Checked::new(
                    SettlementCommand::CancelProject(*project_id),
                    ActionEffect::ProjectCanceled {
                        project_id: *project_id,
                    },
                )
                .target(Target::Project(*project_id)))
            }
            SettlementAction::SetBuildingWorkers {
                building_id,
                workers,
            } => {
                let building = own_building(settlement, *building_id)?;
                let max_workers = building.definition().max_workers();
                if *workers > max_workers {
                    return Err(ActionError::WorkersAboveCapacity {
                        building_id: *building_id,
                        workers: *workers,
                        max_workers,
                    });
                }
                let load = Building::load_for_workers(*workers, max_workers).ok_or(
                    ActionError::UnsupportedWorkerCount {
                        building_id: *building_id,
                        workers: *workers,
                    },
                )?;
                batch.ensure_free(Target::Workers(*building_id))?;
                if *workers > batch.worker_limit(settlement, *building_id, max_workers) {
                    let before = batch.staff_before[building_id];
                    return Err(ActionError::InsufficientWorkers {
                        requested: batch.total_staff - u64::from(before) + u64::from(*workers),
                        available: settlement.population(),
                    });
                }
                let mut checked = Checked::new(
                    SettlementCommand::SetBuildingStaff {
                        building_id: *building_id,
                        load,
                    },
                    ActionEffect::WorkersSet {
                        building_id: *building_id,
                        workers: *workers,
                    },
                )
                .target(Target::Workers(*building_id));
                checked.staff = Some((*building_id, *workers));
                Ok(checked)
            }
            SettlementAction::SetStorageOrder { order } => {
                StorageOrder::validate(order)
                    .map_err(|error| ActionError::InvalidStorageOrder { error })?;
                batch.ensure_free(Target::StorageOrder)?;
                Ok(Checked::new(
                    SettlementCommand::SetStorageOrder(order.clone()),
                    ActionEffect::StorageOrderSet,
                )
                .target(Target::StorageOrder))
            }
            SettlementAction::PublishMarketOffers { offers } => {
                let offers = resolve_offers(offers);
                batch.ensure_free(Target::Market)?;
                let (goods, gold) = offer_demand(&offers);
                batch.check_market(settlement, &goods, gold)?;
                let mut checked = Checked::new(
                    SettlementCommand::PublishMarketOffers(offers),
                    ActionEffect::MarketOffersPublished,
                )
                .target(Target::Market);
                checked.market = Some((goods, gold));
                Ok(checked)
            }
            SettlementAction::ClearMarketOffers => {
                batch.ensure_free(Target::Market)?;
                let mut checked = Checked::new(
                    SettlementCommand::ClearMarketOffers,
                    ActionEffect::MarketOffersCleared,
                )
                .target(Target::Market);
                checked.market = Some((BTreeMap::new(), 0));
                Ok(checked)
            }
            SettlementAction::OrderCaravan {
                receiver_id,
                offers,
            } => self.check_caravan(settlement, *receiver_id, offers, batch),
        }
    }

    fn catalog_definition(
        &self,
        building_type: &BuildingTypeId,
    ) -> Result<Arc<BuildingDefinition>, ActionError> {
        self.world
            .building_catalog()
            .get_blueprint(building_type.as_str())
            .ok_or_else(|| ActionError::UnknownBuildingType {
                building_type: building_type.clone(),
            })
    }

    /// The same placement rules as construction: own province, land, empty tile.
    pub(crate) fn check_construction_site(
        &self,
        settlement_id: SettlementId,
        position: Position,
    ) -> Result<(), ActionError> {
        self.world
            .validate_construction_site(settlement_id, position)
            .map_err(|error| match error {
                WorldError::InvalidMapPosition(_)
                | WorldError::Project(ProjectError::Territory(
                    TerritoryError::InvalidMapPosition(_),
                )) => ActionError::InvalidPosition { position },
                WorldError::Project(ProjectError::Territory(TerritoryError::OceanTile(_))) => {
                    ActionError::OceanTile { position }
                }
                WorldError::Project(ProjectError::Territory(TerritoryError::Occupied {
                    ..
                })) => ActionError::TileOccupied { position },
                _ => ActionError::OutsideOwnProvince { position },
            })
    }

    /// Receiver rules for caravans: another registered settlement.
    pub(in crate::application) fn check_caravan_receiver(
        &self,
        settlement: &Settlement,
        receiver_id: SettlementId,
    ) -> Result<(), ActionError> {
        if receiver_id == settlement.id() {
            return Err(ActionError::TradeWithSelf);
        }
        if self.world.settlement(receiver_id).is_none() {
            return Err(ActionError::UnknownSettlement {
                settlement_id: receiver_id,
            });
        }
        Ok(())
    }

    fn check_caravan(
        &self,
        settlement: &Settlement,
        receiver_id: SettlementId,
        offers: &[OfferSpec],
        batch: &Batch,
    ) -> Result<Checked, ActionError> {
        let resolved = resolve_offers(offers);
        self.check_caravan_receiver(settlement, receiver_id)?;
        let (goods, gold) = offer_demand(&resolved);
        // The engine skips orders that carry neither goods nor gold.
        if goods.is_empty() && gold == 0 {
            return Err(ActionError::NothingToCarry);
        }
        for (&resource, &requested) in &goods {
            let available = batch.caravan_room(settlement, resource);
            if requested > available {
                return Err(ActionError::InsufficientStock {
                    resource,
                    requested,
                    available: room_u32(available),
                });
            }
        }
        let available = batch.caravan_gold_room(settlement);
        if gold > available {
            return Err(ActionError::InsufficientGold {
                requested: gold,
                available: room_u32(available),
            });
        }
        let mut checked = Checked::new(
            SettlementCommand::OrderCaravan {
                receiver_id,
                offers: resolved,
            },
            ActionEffect::CaravanOrdered { receiver_id },
        );
        checked.caravan_goods = goods;
        checked.caravan_gold = gold;
        Ok(checked)
    }
}

fn own_building(
    settlement: &Settlement,
    building_id: BuildingId,
) -> Result<&Building, ActionError> {
    settlement
        .buildings()
        .find(|building| building.instance_id() == building_id)
        .ok_or(ActionError::UnknownBuilding { building_id })
}

fn resolve_offers(offers: &[OfferSpec]) -> HashMap<ResourceType, TradeOffer> {
    offers
        .iter()
        .map(|offer| (offer.resource, offer.to_offer()))
        .collect()
}
