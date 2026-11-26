//! Maps decision values to live domain operations and their reported outcomes.
use super::{ActionEffect, ActionError, SettlementAction, input_validation::resolve_offers};
use crate::domain::buildings::definition::BuildingDefinition;
use crate::domain::ids::{BuildingTypeId, ProjectId, SettlementId};
use crate::domain::settlement::{BuildingCommandError, TradeCommandError, projects::ProjectError};
use crate::domain::spatial::{Position, TerritoryError};
use crate::domain::world::{World, WorldError};
use std::sync::Arc;

pub(crate) struct CommandDispatch<'a> {
    world: &'a mut World,
}

impl<'a> CommandDispatch<'a> {
    pub(crate) fn new(world: &'a mut World) -> Self {
        Self { world }
    }

    pub(super) fn require_settlement(&self, id: SettlementId) -> Result<(), WorldError> {
        self.world
            .settlement(id)
            .map(|_| ())
            .ok_or(WorldError::MissingSettlement(id))
    }

    pub(super) fn execute(
        &mut self,
        id: SettlementId,
        action: &SettlementAction,
    ) -> Result<ActionEffect, ActionError> {
        match action {
            SettlementAction::Construct {
                building_type,
                position,
            } => {
                if position.x < 0 || position.y < 0 {
                    return Err(ActionError::InvalidPosition {
                        position: *position,
                    });
                }
                let definition = self.definition(building_type)?;
                let project_id = self.next_project_id(id)?;
                let cost = definition.construction_cost().clone();
                self.world
                    .queue_construction(id, project_id, *position, definition, cost)
                    .map_err(|error| project_error(error, Some(*position)))?;
                Ok(ActionEffect::ProjectQueued { project_id })
            }
            SettlementAction::Upgrade {
                building_id,
                building_type,
            } => {
                let building = self
                    .world
                    .settlement(id)
                    .expect("settlement checked")
                    .buildings()
                    .find(|building| building.instance_id() == *building_id)
                    .ok_or(ActionError::UnknownBuilding {
                        building_id: *building_id,
                    })?;
                let definition = self.definition(building_type)?;
                if building.definition().id() == definition.id() {
                    return Err(ActionError::SameBuildingType {
                        building_id: *building_id,
                    });
                }
                let position = building.position();
                let project_id = self.next_project_id(id)?;
                let cost = definition.construction_cost().clone();
                self.world
                    .queue_upgrade(id, project_id, *building_id, definition, cost)
                    .map_err(|error| project_error(error, Some(position)))?;
                Ok(ActionEffect::ProjectQueued { project_id })
            }
            SettlementAction::CancelProject { project_id } => {
                self.world
                    .cancel_project(id, *project_id)
                    .map_err(|error| project_error(error, None))?;
                Ok(ActionEffect::ProjectCanceled {
                    project_id: *project_id,
                })
            }
            SettlementAction::SetBuildingWorkers {
                building_id,
                workers,
            } => {
                self.world
                    .settlement_mut(id)
                    .expect("settlement checked")
                    .set_building_worker_count(*building_id, *workers)
                    .map_err(building_error)?;
                Ok(ActionEffect::WorkersSet {
                    building_id: *building_id,
                    workers: *workers,
                })
            }
            SettlementAction::SetStorageOrder { order } => {
                self.world
                    .settlement_mut(id)
                    .expect("settlement checked")
                    .set_storage_order(order)
                    .map_err(|error| ActionError::InvalidStorageOrder { error })?;
                Ok(ActionEffect::StorageOrderSet)
            }
            SettlementAction::PublishMarketOffers { offers } => {
                let offers = resolve_offers(offers)?;
                self.world
                    .settlement_mut(id)
                    .expect("settlement checked")
                    .publish_trade_offers(offers)
                    .map_err(trade_error)?;
                Ok(ActionEffect::MarketOffersPublished)
            }
            SettlementAction::ClearMarketOffers => {
                self.world
                    .settlement_mut(id)
                    .expect("settlement checked")
                    .clear_trade_offers();
                Ok(ActionEffect::MarketOffersCleared)
            }
            SettlementAction::OrderCaravan {
                receiver_id,
                offers,
            } => {
                let offers = resolve_offers(offers)?;
                self.world
                    .order_caravan(id, *receiver_id, offers)
                    .map_err(trade_error)?;
                Ok(ActionEffect::CaravanOrdered {
                    receiver_id: *receiver_id,
                })
            }
        }
    }

    fn definition(&self, id: &BuildingTypeId) -> Result<Arc<BuildingDefinition>, ActionError> {
        self.world
            .building_catalog()
            .get_blueprint(id.as_str())
            .ok_or_else(|| ActionError::UnknownBuildingType {
                building_type: id.clone(),
            })
    }

    fn next_project_id(&self, id: SettlementId) -> Result<ProjectId, ActionError> {
        self.world
            .settlement(id)
            .expect("settlement checked")
            .next_project_id()
            .ok_or(ActionError::ProjectIdsExhausted)
    }
}

fn project_error(error: WorldError, site: Option<Position>) -> ActionError {
    match error {
        WorldError::Project(ProjectError::UnknownMaterial(resource)) => {
            ActionError::UnknownMaterial { resource }
        }
        WorldError::Project(ProjectError::MissingBuilding(building_id)) => {
            ActionError::UnknownBuilding { building_id }
        }
        WorldError::Project(ProjectError::BuildingAlreadyUpgrading(building_id)) => {
            ActionError::BuildingAlreadyUpgrading { building_id }
        }
        WorldError::Project(ProjectError::MissingProject(project_id)) => {
            ActionError::UnknownProject { project_id }
        }
        WorldError::RequiredDepositMissing { position, resource }
        | WorldError::Project(ProjectError::Territory(TerritoryError::TileDepositRequired {
            position,
            resource,
        })) => ActionError::RequiredDepositMissing { position, resource },
        WorldError::InvalidMapPosition(position)
        | WorldError::Project(ProjectError::Territory(TerritoryError::InvalidMapPosition(
            position,
        ))) => ActionError::InvalidPosition { position },
        WorldError::Project(ProjectError::Territory(TerritoryError::OceanTile(position))) => {
            ActionError::OceanTile { position }
        }
        WorldError::Project(ProjectError::Territory(TerritoryError::CoastRequired(position))) => {
            ActionError::CoastRequired { position }
        }
        WorldError::Project(ProjectError::Territory(TerritoryError::ForbiddenBiome(position))) => {
            ActionError::ForbiddenBiome { position }
        }
        WorldError::Project(ProjectError::Territory(TerritoryError::ProvinceDepositRequired {
            position,
            resource,
        })) => ActionError::ProvinceDepositRequired { position, resource },
        WorldError::Project(ProjectError::Territory(TerritoryError::SiteRequirementNotMet {
            position,
            fact,
        })) => ActionError::SiteRequirementNotMet { position, fact },
        WorldError::Project(ProjectError::Territory(TerritoryError::Occupied {
            position, ..
        })) => ActionError::TileOccupied { position },
        WorldError::MissingProvince(_) | WorldError::SettlementOutsideProvince { .. } => {
            ActionError::OutsideOwnProvince {
                position: site.expect("placement has a site"),
            }
        }
        _ => ActionError::ExecutionFailed,
    }
}

fn building_error(error: BuildingCommandError) -> ActionError {
    match error {
        BuildingCommandError::MissingBuilding(building_id) => {
            ActionError::UnknownBuilding { building_id }
        }
        BuildingCommandError::WorkersAboveCapacity {
            building_id,
            workers,
            max_workers,
        } => ActionError::WorkersAboveCapacity {
            building_id,
            workers,
            max_workers,
        },
        BuildingCommandError::UnsupportedWorkerCount {
            building_id,
            workers,
        } => ActionError::UnsupportedWorkerCount {
            building_id,
            workers,
        },
        BuildingCommandError::InsufficientWorkers {
            requested,
            available,
        } => ActionError::InsufficientWorkers {
            requested,
            available,
        },
        _ => ActionError::ExecutionFailed,
    }
}

pub(super) fn trade_error(error: TradeCommandError) -> ActionError {
    match error {
        TradeCommandError::UnknownMaterial(resource) => ActionError::UnknownMaterial { resource },
        TradeCommandError::LabourIsNotTradable => ActionError::LabourIsNotTradable,
        TradeCommandError::ZeroUnits(resource) => ActionError::ZeroUnits { resource },
        TradeCommandError::EmptyOrder => ActionError::EmptyOrder,
        TradeCommandError::NothingToCarry => ActionError::NothingToCarry,
        TradeCommandError::TradeWithSelf => ActionError::TradeWithSelf,
        TradeCommandError::TradeBlocked {
            receiver_id,
            nation_id,
        } => ActionError::TradeBlocked {
            receiver_id,
            nation_id,
        },
        TradeCommandError::SettlementUnderAttack(settlement_id) => {
            ActionError::SettlementUnderAttack { settlement_id }
        }
        TradeCommandError::UnknownSender(settlement_id)
        | TradeCommandError::UnknownReceiver(settlement_id) => {
            ActionError::UnknownSettlement { settlement_id }
        }
        TradeCommandError::InsufficientStock {
            resource,
            requested,
            available,
        } => ActionError::InsufficientStock {
            resource,
            requested,
            available,
        },
        TradeCommandError::InsufficientGold {
            requested,
            available,
        } => ActionError::InsufficientGold {
            requested,
            available,
        },
    }
}
