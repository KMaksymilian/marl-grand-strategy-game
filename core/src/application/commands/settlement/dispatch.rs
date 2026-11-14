//! Single command facade: scheduling never borrows world internals to dispatch actions.
use crate::application::commands::settlement::prepared::SettlementCommand;
use crate::domain::economy::storage_order::StorageOrderError;
use crate::domain::ids::SettlementId;
use crate::domain::settlement::TradeCommandError;
use crate::domain::world::{World, WorldError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CommandError {
    World(WorldError),
    StorageOrder(StorageOrderError),
    Trade(TradeCommandError),
}

/// The handler can read the world and execute prepared domain operations.
pub(crate) struct CommandDispatch<'a> {
    world: &'a mut World,
}

impl<'a> CommandDispatch<'a> {
    pub(crate) fn new(world: &'a mut World) -> Self {
        Self { world }
    }

    pub(super) fn world(&self) -> &World {
        self.world
    }

    pub(super) fn execute(
        &mut self,
        id: SettlementId,
        command: &SettlementCommand,
    ) -> Result<(), CommandError> {
        let world = &mut *self.world;
        match command {
            SettlementCommand::Construct {
                project_id,
                position,
                definition,
                cost,
            } => world
                .queue_construction(id, *project_id, *position, definition.clone(), cost.clone())
                .map_err(CommandError::World),
            SettlementCommand::Upgrade {
                project_id,
                building_id,
                definition,
                cost,
            } => world
                .queue_upgrade(
                    id,
                    *project_id,
                    *building_id,
                    definition.clone(),
                    cost.clone(),
                )
                .map_err(CommandError::World),
            SettlementCommand::CancelProject(project_id) => world
                .cancel_project(id, *project_id)
                .map_err(CommandError::World),
            SettlementCommand::SetBuildingStaff { building_id, load } => world
                .settlement_mut(id)
                .map_err(CommandError::World)?
                .set_building_staff(*building_id, *load)
                .map_err(|error| CommandError::World(WorldError::Building(error))),
            SettlementCommand::SetStorageOrder(materials) => world
                .settlement_mut(id)
                .map_err(CommandError::World)?
                .set_storage_order(materials)
                .map_err(CommandError::StorageOrder),
            SettlementCommand::PublishMarketOffers(offers) => world
                .settlement_mut(id)
                .map_err(CommandError::World)?
                .publish_trade_offers(offers.clone())
                .map_err(CommandError::Trade),
            SettlementCommand::ClearMarketOffers => {
                world
                    .settlement_mut(id)
                    .map_err(CommandError::World)?
                    .clear_trade_offers();
                Ok(())
            }
            SettlementCommand::OrderCaravan {
                receiver_id,
                offers,
            } => world
                .settlement_mut(id)
                .map_err(CommandError::World)?
                .order_caravan(*receiver_id, offers.clone())
                .map_err(CommandError::Trade),
        }
    }
}
