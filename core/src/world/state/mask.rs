//! Action masks: every listed option is confirmed by the action validator.
use super::validation::Batch;
use super::{World, WorldError};
use crate::economy::ResourceType;
use crate::ids::{BuildingTypeId, SettlementId};
use crate::settlement::Settlement;
use crate::settlement::action_mask::{ActionMask, ConstructOption, UpgradeOption, WorkerRange};
use crate::settlement::decision::{ActionKind, OfferSide, OfferSpec, SettlementAction};

impl World {
    pub(crate) fn action_mask(
        &self,
        settlement_id: SettlementId,
    ) -> Result<ActionMask, WorldError> {
        let settlement = self
            .settlement(settlement_id)
            .ok_or(WorldError::MissingSettlement(settlement_id))?;
        let province_id = self
            .settlement_province(settlement_id)
            .ok_or(WorldError::MissingSettlement(settlement_id))?;
        let batch = Batch::new(settlement);
        let allows = |action: SettlementAction| {
            self.check_single_action(settlement, &action, &batch)
                .is_ok()
        };
        let types: Vec<_> = self
            .building_catalog
            .ids()
            .into_iter()
            .map(BuildingTypeId::new)
            .collect();
        let mut buildings: Vec<_> = settlement.buildings().collect();
        buildings.sort_by_key(|building| building.instance_id());

        let construct: Vec<_> = self
            .province_positions(province_id)
            .into_iter()
            .filter_map(|position| {
                let building_types: Vec<_> = types
                    .iter()
                    .filter(|building_type| {
                        allows(SettlementAction::Construct {
                            building_type: (*building_type).clone(),
                            position,
                        })
                    })
                    .cloned()
                    .collect();
                (!building_types.is_empty()).then_some(ConstructOption {
                    position,
                    building_types,
                })
            })
            .collect();

        let upgrade: Vec<_> = buildings
            .iter()
            .filter_map(|building| {
                let building_types: Vec<_> = types
                    .iter()
                    .filter(|building_type| {
                        allows(SettlementAction::Upgrade {
                            building_id: building.instance_id(),
                            building_type: (*building_type).clone(),
                        })
                    })
                    .cloned()
                    .collect();
                (!building_types.is_empty()).then_some(UpgradeOption {
                    building_id: building.instance_id(),
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

        // The cheapest possible order checks only the receiver rules.
        let probe = vec![OfferSpec {
            resource: ResourceType::MATERIALS[0],
            side: OfferSide::Buy,
            units: 1,
            price_per_unit: 0,
        }];
        let caravan_receivers: Vec<_> = self
            .settlements()
            .map(Settlement::id)
            .filter(|&receiver_id| {
                allows(SettlementAction::OrderCaravan {
                    receiver_id,
                    offers: probe.clone(),
                })
            })
            .collect();

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
                    ActionKind::OrderCaravan => !caravan_receivers.is_empty(),
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
            caravan_sell_limits: ResourceType::MATERIALS
                .iter()
                .map(|&resource| (resource, settlement.inventory().amount(resource)))
                .collect(),
            caravan_gold_limit: settlement.gold(),
        })
    }
}
