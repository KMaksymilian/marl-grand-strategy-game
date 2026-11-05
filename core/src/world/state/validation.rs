//! Read-only validation of agent decisions against the authoritative world.
//!
//! Policy: actions are checked in submission order and each one is accepted or
//! rejected on its own. Accepted actions are resolved to `SettlementCommand`s with
//! catalog definitions and costs, simulation-assigned project IDs and exact building
//! loads. A later action that targets the same object as an earlier accepted action
//! is rejected as a conflict. Validation never changes the world.
use super::{World, WorldError};
use crate::buildings::Building;
use crate::buildings::definition::BuildingDefinition;
use crate::economy::ResourceType;
use crate::economy::storage_order::StorageOrder;
use crate::ids::{BuildingId, BuildingTypeId, ProjectId, SettlementId};
use crate::settlement::Settlement;
use crate::settlement::command::SettlementCommand;
use crate::settlement::decision::{
    ActionError, OfferSide, OfferSpec, RejectedAction, SettlementAction, SettlementDecision,
};
use crate::settlement::projects::ProjectResult;
use crate::spatial::{Position, TerritoryError, TileOccupancy};
use crate::trade::offer::TradeOffer;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// An accepted action, resolved to the command that executes it.
#[derive(Debug, Clone)]
pub struct AcceptedAction {
    /// Position in `SettlementDecision::actions`.
    pub index: usize,
    pub command: SettlementCommand,
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
    target: Option<Target>,
    new_project: bool,
    staff: Option<(BuildingId, u32)>,
    caravan_goods: BTreeMap<ResourceType, u64>,
    caravan_gold: u64,
}

impl Checked {
    fn new(command: SettlementCommand) -> Self {
        Self {
            command,
            target: None,
            new_project: false,
            staff: None,
            caravan_goods: BTreeMap::new(),
            caravan_gold: 0,
        }
    }

    fn target(mut self, target: Target) -> Self {
        self.target = Some(target);
        self
    }
}

/// Settlement state as changed by the decision's earlier accepted actions.
pub(super) struct Batch {
    claimed: HashMap<Target, usize>,
    next_project: u64,
    /// Staff requested by each building before this decision.
    staff_before: HashMap<BuildingId, u32>,
    total_staff: u64,
    caravan_goods: BTreeMap<ResourceType, u64>,
    caravan_gold: u64,
}

impl Batch {
    pub(super) fn new(settlement: &Settlement) -> Self {
        let staff_before: HashMap<_, _> = settlement
            .buildings()
            .map(|building| (building.instance_id(), requested_staff(building)))
            .collect();
        Self {
            claimed: HashMap::new(),
            next_project: settlement
                .next_project_id()
                .map_or(u64::from(u32::MAX) + 1, |id| u64::from(id.0)),
            total_staff: staff_before.values().map(|&staff| u64::from(staff)).sum(),
            staff_before,
            caravan_goods: BTreeMap::new(),
            caravan_gold: 0,
        }
    }

    fn ensure_free(&self, target: Target) -> Result<(), ActionError> {
        match self.claimed.get(&target) {
            Some(&with_index) => Err(ActionError::Conflict { with_index }),
            None => Ok(()),
        }
    }

    /// Highest staff the building may request. Raising staff must keep all
    /// buildings within the population; keeping or lowering staff is always allowed.
    pub(super) fn worker_limit(
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
    }
}

fn requested_staff(building: &Building) -> u32 {
    building
        .calculate_demand()
        .get(&ResourceType::Labour)
        .copied()
        .unwrap_or(0)
}

impl World {
    /// Checks every action without changing the world. Fails as a whole only when
    /// the deciding settlement does not exist.
    pub fn validate_decision(
        &self,
        decision: &SettlementDecision,
    ) -> Result<ValidatedDecision, WorldError> {
        let settlement = self
            .settlement(decision.settlement_id)
            .ok_or(WorldError::MissingSettlement(decision.settlement_id))?;
        let mut batch = Batch::new(settlement);
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        for (index, action) in decision.actions.iter().enumerate() {
            match self.check_action(settlement, action, &batch) {
                Ok(checked) => {
                    batch.accept(index, &checked);
                    accepted.push(AcceptedAction {
                        index,
                        command: checked.command,
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
    pub(super) fn check_single_action(
        &self,
        settlement: &Settlement,
        action: &SettlementAction,
        batch: &Batch,
    ) -> Result<(), ActionError> {
        self.check_action(settlement, action, batch).map(|_| ())
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
                let mut checked = Checked::new(SettlementCommand::Construct {
                    project_id: batch.project_id()?,
                    position: *position,
                    cost: definition.construction_cost().clone(),
                    definition,
                })
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
                self.validate_settlement_position(settlement.id(), building.position())
                    .map_err(|_| ActionError::OutsideOwnProvince {
                        position: building.position(),
                    })?;
                batch.ensure_free(Target::Upgrade(*building_id))?;
                let mut checked = Checked::new(SettlementCommand::Upgrade {
                    project_id: batch.project_id()?,
                    building_id: *building_id,
                    cost: definition.construction_cost().clone(),
                    definition,
                })
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
                Ok(Checked::new(SettlementCommand::CancelProject(*project_id))
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
                let mut checked = Checked::new(SettlementCommand::SetBuildingStaff {
                    building_id: *building_id,
                    load,
                })
                .target(Target::Workers(*building_id));
                checked.staff = Some((*building_id, *workers));
                Ok(checked)
            }
            SettlementAction::SetStorageOrder { order } => {
                StorageOrder::validate(order)
                    .map_err(|error| ActionError::InvalidStorageOrder { error })?;
                batch.ensure_free(Target::StorageOrder)?;
                Ok(
                    Checked::new(SettlementCommand::SetStorageOrder(order.clone()))
                        .target(Target::StorageOrder),
                )
            }
            SettlementAction::PublishMarketOffers { offers } => {
                let offers = trade_offers(offers)?;
                batch.ensure_free(Target::Market)?;
                Ok(Checked::new(SettlementCommand::PublishMarketOffers(offers))
                    .target(Target::Market))
            }
            SettlementAction::ClearMarketOffers => {
                batch.ensure_free(Target::Market)?;
                Ok(Checked::new(SettlementCommand::ClearMarketOffers).target(Target::Market))
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
        self.building_catalog
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
        match self.validate_settlement_position(settlement_id, position) {
            Ok(()) => {}
            Err(WorldError::InvalidMapPosition(_)) => {
                return Err(ActionError::InvalidPosition { position });
            }
            Err(_) => return Err(ActionError::OutsideOwnProvince { position }),
        }
        self.map
            .validate_building_position(position)
            .map_err(|error| match error {
                TerritoryError::OceanTile(_) => ActionError::OceanTile { position },
                _ => ActionError::InvalidPosition { position },
            })?;
        if self.territory.tile_at(position).occupancy() != TileOccupancy::Empty {
            return Err(ActionError::TileOccupied { position });
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
        let resolved = trade_offers(offers)?;
        if offers.is_empty() {
            return Err(ActionError::EmptyOrder);
        }
        if receiver_id == settlement.id() {
            return Err(ActionError::TradeWithSelf);
        }
        if self.settlement(receiver_id).is_none() {
            return Err(ActionError::UnknownSettlement {
                settlement_id: receiver_id,
            });
        }
        let mut checked = Checked::new(SettlementCommand::OrderCaravan {
            receiver_id,
            offers: resolved,
        });
        for offer in offers {
            let amount = u64::from(offer.units);
            match offer.side {
                OfferSide::Sell => {
                    *checked.caravan_goods.entry(offer.resource).or_default() += amount
                }
                OfferSide::Buy => {
                    checked.caravan_gold += amount * u64::from(offer.price_per_unit);
                }
            }
        }
        for (&resource, &amount) in &checked.caravan_goods {
            let requested = batch.caravan_goods.get(&resource).copied().unwrap_or(0) + amount;
            let available = settlement.inventory().amount(resource);
            if requested > u64::from(available) {
                return Err(ActionError::InsufficientStock {
                    resource,
                    requested,
                    available,
                });
            }
        }
        let requested = batch.caravan_gold + checked.caravan_gold;
        if requested > u64::from(settlement.gold()) {
            return Err(ActionError::InsufficientGold {
                requested,
                available: settlement.gold(),
            });
        }
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

/// Offer rules shared by the market and caravans.
fn trade_offers(offers: &[OfferSpec]) -> Result<HashMap<ResourceType, TradeOffer>, ActionError> {
    let mut resolved = HashMap::new();
    for offer in offers {
        if offer.resource == ResourceType::Labour {
            return Err(ActionError::LabourIsNotTradable);
        }
        if offer.units == 0 {
            return Err(ActionError::ZeroUnits {
                resource: offer.resource,
            });
        }
        if resolved.insert(offer.resource, offer.to_offer()).is_some() {
            return Err(ActionError::DuplicateOffer {
                resource: offer.resource,
            });
        }
    }
    Ok(resolved)
}
