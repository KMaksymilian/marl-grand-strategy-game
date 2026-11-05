//! Command values independent of the simulation scheduler and map implementation.
use crate::buildings::definition::BuildingDefinition;
use crate::economy::{ResourceType, cost::ProjectCost};
use crate::ids::{BuildingId, ProjectId, SettlementId};
use crate::spatial::Position;
use crate::trade::offer::TradeOffer;
use std::collections::HashMap;
use std::sync::Arc;

/// Explicit caller-supplied commands; this does not implement an AI policy.
#[derive(Debug, Clone)]
pub enum SettlementCommand {
    Construct {
        project_id: ProjectId,
        position: Position,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    },
    Upgrade {
        project_id: ProjectId,
        building_id: BuildingId,
        definition: Arc<BuildingDefinition>,
        cost: ProjectCost,
    },
    CancelProject(ProjectId),
    SetBuildingLoad {
        building_id: BuildingId,
        load: f32,
    },
    SetBuildingActive {
        building_id: BuildingId,
        active: bool,
    },
    SetStorageOrder(Vec<ResourceType>),
    PublishMarketOffers(HashMap<ResourceType, TradeOffer>),
    ClearMarketOffers,
    OrderCaravan {
        receiver_id: SettlementId,
        offers: HashMap<ResourceType, TradeOffer>,
    },
}
