//! Command values independent of the simulation scheduler and map implementation.
use crate::domain::buildings::definition::BuildingDefinition;
use crate::domain::economy::{ResourceType, cost::ProjectCost};
use crate::domain::ids::{BuildingId, ProjectId, SettlementId};
use crate::domain::spatial::Position;
use crate::domain::trade::offer::TradeOffer;
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
    /// Activate the building and set its load; used for agent staff decisions.
    SetBuildingStaff {
        building_id: BuildingId,
        load: f32,
    },
    SetStorageOrder(Vec<ResourceType>),
    PublishMarketOffers(HashMap<ResourceType, TradeOffer>),
    ClearMarketOffers,
    OrderCaravan {
        receiver_id: SettlementId,
        offers: HashMap<ResourceType, TradeOffer>,
    },
}
