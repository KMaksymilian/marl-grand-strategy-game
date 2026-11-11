//! Action masks: every listed option is confirmed by the action validator.
use crate::application::commands::settlement::contract::{ActionKind, SettlementAction};
use crate::application::commands::settlement::preparation::{Batch, DecisionPreparation};
use crate::application::queries::action_mask::{
    ActionMask, ConstructOption, UpgradeOption, WorkerRange,
};
use crate::domain::economy::ResourceType;
use crate::domain::ids::{BuildingTypeId, SettlementId};
use crate::domain::settlement::Settlement;
use crate::domain::world::{World, WorldError};
use std::collections::BTreeMap;

pub(crate) fn legal_actions(
    world: &World,
    settlement_id: SettlementId,
) -> Result<ActionMask, WorldError> {
    let settlement = world
        .settlement(settlement_id)
        .ok_or(WorldError::MissingSettlement(settlement_id))?;
    let province_id = world
        .settlement_province(settlement_id)
        .ok_or(WorldError::MissingSettlement(settlement_id))?;
    let validator = DecisionPreparation::new(world);
    let batch = Batch::new(settlement);
    let allows = |action: SettlementAction| {
        validator
            .check_single_action(settlement, &action, &batch)
            .is_ok()
    };
    let types: Vec<_> = world
        .building_catalog()
        .ids()
        .into_iter()
        .map(BuildingTypeId::new)
        .collect();
    // Catalog types for which `action(building_type)` passes validation.
    let allowed_types = |action: &dyn Fn(BuildingTypeId) -> SettlementAction| -> Vec<_> {
        types
            .iter()
            .filter(|building_type| allows(action((*building_type).clone())))
            .cloned()
            .collect()
    };
    let mut buildings: Vec<_> = settlement.buildings().collect();
    buildings.sort_by_key(|building| building.instance_id());

    let construct: Vec<_> = world
        .province_positions(province_id)
        .into_iter()
        .filter_map(|position| {
            let building_types = allowed_types(&|building_type| SettlementAction::Construct {
                building_type,
                position,
            });
            (!building_types.is_empty()).then_some(ConstructOption {
                position,
                building_types,
            })
        })
        .collect();

    let upgrade: Vec<_> = buildings
        .iter()
        .filter_map(|building| {
            let building_id = building.instance_id();
            let building_types = allowed_types(&|building_type| SettlementAction::Upgrade {
                building_id,
                building_type,
            });
            (!building_types.is_empty()).then_some(UpgradeOption {
                building_id,
                building_types,
            })
        })
        .collect();

    let cancel_project: Vec<_> = settlement
        .active_projects()
        .iter()
        .map(|project| project.id())
        .filter(|&project_id| allows(SettlementAction::CancelProject { project_id }))
        .collect();

    let set_building_workers: Vec<_> = buildings
        .iter()
        .filter(|building| {
            allows(SettlementAction::SetBuildingWorkers {
                building_id: building.instance_id(),
                workers: 0,
            })
        })
        .map(|building| WorkerRange {
            building_id: building.instance_id(),
            max_workers: batch.worker_limit(
                settlement,
                building.instance_id(),
                building.definition().max_workers(),
            ),
        })
        .collect();

    let caravan_receivers: Vec<_> = world
        .settlements()
        .map(Settlement::id)
        .filter(|&receiver_id| {
            validator
                .check_caravan_receiver(settlement, receiver_id)
                .is_ok()
        })
        .collect();
    // What caravans can still be funded with after the market, which goes first.
    let caravan_sell_limits: BTreeMap<_, _> = ResourceType::MATERIALS
        .iter()
        .map(|&resource| {
            let room = batch.caravan_room(settlement, resource);
            (resource, u32::try_from(room).unwrap_or(u32::MAX))
        })
        .collect();
    let caravan_gold_limit = u32::try_from(batch.caravan_gold_room(settlement)).unwrap_or(u32::MAX);
    let caravan_can_carry =
        caravan_sell_limits.values().any(|&units| units > 0) || caravan_gold_limit > 0;

    let available = ActionKind::ALL
        .into_iter()
        .map(|kind| {
            let available = match kind {
                ActionKind::Construct => !construct.is_empty(),
                ActionKind::Upgrade => !upgrade.is_empty(),
                ActionKind::CancelProject => !cancel_project.is_empty(),
                ActionKind::SetBuildingWorkers => !set_building_workers.is_empty(),
                ActionKind::SetStorageOrder => allows(SettlementAction::SetStorageOrder {
                    order: ResourceType::MATERIALS.to_vec(),
                }),
                ActionKind::PublishMarketOffers => {
                    allows(SettlementAction::PublishMarketOffers { offers: vec![] })
                }
                ActionKind::ClearMarketOffers => allows(SettlementAction::ClearMarketOffers),
                ActionKind::OrderCaravan => !caravan_receivers.is_empty() && caravan_can_carry,
            };
            (kind, available)
        })
        .collect();

    Ok(ActionMask {
        available,
        construct,
        upgrade,
        cancel_project,
        set_building_workers,
        storage_materials: ResourceType::MATERIALS.to_vec(),
        tradable_resources: ResourceType::MATERIALS.to_vec(),
        caravan_receivers,
        caravan_sell_limits,
        caravan_gold_limit,
    })
}
