//! Single command facade: scheduling never borrows world internals to dispatch actions.
use super::{World, WorldError};
use crate::economy::storage_order::StorageOrderError;
use crate::ids::SettlementId;
use crate::settlement::TradeCommandError;
use crate::settlement::command::SettlementCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    World(WorldError),
    StorageOrder(StorageOrderError),
    Trade(TradeCommandError),
}

impl World {
    pub fn execute_settlement_command(
        &mut self,
        id: SettlementId,
        command: &SettlementCommand,
    ) -> Result<(), CommandError> {
        match command {
            SettlementCommand::Construct {
                project_id,
                position,
                definition,
                cost,
            } => self
                .queue_construction(id, *project_id, *position, definition.clone(), cost.clone())
                .map_err(CommandError::World),
            SettlementCommand::Upgrade {
                project_id,
                building_id,
                definition,
                cost,
            } => self
                .queue_upgrade(
                    id,
                    *project_id,
                    *building_id,
                    definition.clone(),
                    cost.clone(),
                )
                .map_err(CommandError::World),
            SettlementCommand::CancelProject(project_id) => self
                .cancel_project(id, *project_id)
                .map_err(CommandError::World),
            SettlementCommand::SetBuildingLoad { building_id, load } => self
                .settlement_mut(id)
                .map_err(CommandError::World)?
                .set_building_load(*building_id, *load)
                .map_err(|error| CommandError::World(WorldError::Building(error))),
            SettlementCommand::SetBuildingActive {
                building_id,
                active,
            } => self
                .settlement_mut(id)
                .map_err(CommandError::World)?
                .set_building_active(*building_id, *active)
                .map_err(|error| CommandError::World(WorldError::Building(error))),
            SettlementCommand::SetStorageOrder(materials) => self
                .settlement_mut(id)
                .map_err(CommandError::World)?
                .set_storage_order(materials)
                .map_err(CommandError::StorageOrder),
            SettlementCommand::PublishMarketOffers(offers) => self
                .settlement_mut(id)
                .map_err(CommandError::World)?
                .publish_trade_offers(offers.clone())
                .map_err(CommandError::Trade),
            SettlementCommand::ClearMarketOffers => {
                self.settlement_mut(id)
                    .map_err(CommandError::World)?
                    .clear_trade_offers();
                Ok(())
            }
            SettlementCommand::OrderCaravan {
                receiver_id,
                offers,
            } => self
                .settlement_mut(id)
                .map_err(CommandError::World)?
                .order_caravan(*receiver_id, offers.clone())
                .map_err(CommandError::Trade),
        }
    }
}
