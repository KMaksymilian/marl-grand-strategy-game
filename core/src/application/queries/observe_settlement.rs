//! Builds a settlement's observation from the authoritative world.
//! Only the observed settlement, its own province tiles, the public catalog and
//! the IDs of other settlements are read.
use crate::application::commands::settlement::contract::{OfferSide, OfferSpec};
use crate::application::queries::observation::{
    BuildingObservation, BuildingTypeObservation, InventoryObservation, LastTurnObservation,
    ProjectObservation, ProjectTarget, RecipeObservation, ResourceAmounts, SettlementObservation,
    TileObservation, TileState, TradeObservation, TradePartner, WorkforceObservation,
};
use crate::domain::buildings::Building;
use crate::domain::buildings::definition::BuildingBehavior;
use crate::domain::economy::{ResourceInventory, ResourceType};
use crate::domain::ids::{BuildingTypeId, SettlementId};
use crate::domain::settlement::Settlement;
use crate::domain::settlement::projects::ProjectResult;
use crate::domain::settlement::turn_report::TurnReport;
use crate::domain::spatial::TileOccupancy;
use crate::domain::trade::offer::TradeOffer;
use crate::domain::world::{World, WorldError};

pub(crate) fn observe_settlement(
    world: &World,
    settlement_id: SettlementId,
    turn: u64,
    last_turn: Option<&TurnReport>,
) -> Result<SettlementObservation, WorldError> {
    ObservationQuery { world }.observe(settlement_id, turn, last_turn)
}

struct ObservationQuery<'a> {
    world: &'a World,
}

impl ObservationQuery<'_> {
    /// `turn` is the number of completed global turns and `last_turn` the
    /// settlement's report from the latest one, both kept by the coordinator.
    fn observe(
        &self,
        settlement_id: SettlementId,
        turn: u64,
        last_turn: Option<&TurnReport>,
    ) -> Result<SettlementObservation, WorldError> {
        let settlement = self
            .world
            .settlement(settlement_id)
            .ok_or(WorldError::MissingSettlement(settlement_id))?;
        let province_id = self
            .world
            .settlement_province(settlement_id)
            .ok_or(WorldError::MissingSettlement(settlement_id))?;
        let mut buildings: Vec<_> = settlement.buildings().map(building_observation).collect();
        buildings.sort_by_key(|building| building.building_id);
        Ok(SettlementObservation {
            turn,
            settlement_id,
            province_id,
            population: settlement.population(),
            gold: settlement.gold(),
            inventory: InventoryObservation {
                capacity: settlement.inventory().capacity(),
                stored: materials(settlement.inventory()),
                stored_total: settlement.inventory().total_amount(),
                pending: materials(settlement.pending_inventory()),
                storage_order: settlement.storage_order().to_vec(),
            },
            workforce: WorkforceObservation {
                available: settlement.population(),
                requested: buildings
                    .iter()
                    .map(|building| u64::from(building.requested_workers))
                    .sum(),
                employed: buildings
                    .iter()
                    .map(|building| u64::from(building.current_workers))
                    .sum(),
            },
            needs_per_turn: settlement.needs_per_turn().into_iter().collect(),
            buildings,
            projects: projects(settlement),
            next_project_id: settlement.next_project_id(),
            trade: TradeObservation {
                market_offers: market_offers(settlement),
                caravans_in_transit: self
                    .world
                    .caravans
                    .iter()
                    .filter(|caravan| caravan.sender_id == settlement_id)
                    .count(),
                caravans_returning: settlement.returning_caravans().len(),
            },
            last_turn: last_turn.map(last_turn_observation),
            territory: self.own_tiles(settlement_id, province_id),
            trade_partners: self.trade_partners(settlement_id),
            building_types: self.building_types(),
        })
    }

    fn own_tiles(&self, settlement_id: SettlementId, province_id: usize) -> Vec<TileObservation> {
        self.world
            .province_positions(province_id)
            .into_iter()
            .filter_map(|position| {
                let area = &self.world.map().terrain[self.world.map().tile_index(position)?];
                let occupancy = match self.world.territory().tile_at(position).occupancy() {
                    TileOccupancy::Empty => TileState::Empty,
                    TileOccupancy::Building(reference)
                        if reference.settlement_id == settlement_id =>
                    {
                        TileState::OwnBuilding {
                            building_id: reference.building_id,
                        }
                    }
                    TileOccupancy::Construction(reference)
                        if reference.settlement_id == settlement_id =>
                    {
                        TileState::OwnConstruction {
                            project_id: reference.project_id,
                        }
                    }
                    TileOccupancy::Building(_) | TileOccupancy::Construction(_) => {
                        TileState::Foreign
                    }
                };
                Some(TileObservation {
                    position,
                    elevation: area.determine_elevation(),
                    moisture: area.determine_moisture(),
                    occupancy,
                    buildable: self
                        .world
                        .validate_construction_site(settlement_id, position)
                        .is_ok(),
                })
            })
            .collect()
    }

    fn trade_partners(&self, settlement_id: SettlementId) -> Vec<TradePartner> {
        self.world
            .settlements()
            .filter(|settlement| settlement.id() != settlement_id)
            .filter_map(|settlement| {
                let id = settlement.id();
                Some(TradePartner {
                    settlement_id: id,
                    province_id: self.world.settlement_province(id)?,
                })
            })
            .collect()
    }

    fn building_types(&self) -> Vec<BuildingTypeObservation> {
        self.world
            .building_catalog()
            .ids()
            .into_iter()
            .filter_map(|id| self.world.building_catalog().get_blueprint(id))
            .map(|definition| BuildingTypeObservation {
                building_type: BuildingTypeId::new(definition.id()),
                name: definition.name().to_owned(),
                tier: definition.tier(),
                max_workers: definition.max_workers(),
                construction_resources: positive(
                    definition.construction_cost().needed_resources.iter(),
                ),
                construction_turns: definition.construction_cost().turn_cost,
                recipes: definition
                    .behaviors()
                    .iter()
                    .filter_map(|behavior| match behavior {
                        BuildingBehavior::Production { inputs, outputs } => {
                            Some(RecipeObservation {
                                inputs: summed(inputs),
                                outputs: summed(outputs),
                            })
                        }
                        BuildingBehavior::StatModifier { .. } => None,
                    })
                    .collect(),
            })
            .collect()
    }
}

fn materials(inventory: &ResourceInventory) -> ResourceAmounts {
    positive(
        inventory
            .resources()
            .iter()
            .filter(|(resource, _)| **resource != ResourceType::Labour),
    )
}

fn positive<'a>(amounts: impl Iterator<Item = (&'a ResourceType, &'a u32)>) -> ResourceAmounts {
    amounts
        .filter(|(_, amount)| **amount > 0)
        .map(|(&resource, &amount)| (resource, amount))
        .collect()
}

fn summed(entries: &[(ResourceType, u32)]) -> ResourceAmounts {
    let mut totals = ResourceAmounts::new();
    for &(resource, amount) in entries {
        *totals.entry(resource).or_default() += amount;
    }
    totals
}

fn building_observation(building: &Building) -> BuildingObservation {
    let max_workers = building.definition().max_workers();
    BuildingObservation {
        building_id: building.instance_id(),
        building_type: BuildingTypeId::new(building.definition().id()),
        position: building.position(),
        active: building.is_active(),
        max_workers,
        requested_workers: building.requested_workers(),
        current_workers: building.current_workers(),
        location_productivity: building.location_productivity(),
    }
}

fn projects(settlement: &Settlement) -> Vec<ProjectObservation> {
    settlement
        .active_projects()
        .iter()
        .map(|project| ProjectObservation {
            project_id: project.id(),
            target: match project.result() {
                ProjectResult::ConstructBuilding {
                    position,
                    definition,
                } => ProjectTarget::Construct {
                    building_type: BuildingTypeId::new(definition.id()),
                    position: *position,
                },
                ProjectResult::UpgradeBuilding {
                    target_instance_id,
                    new_definition,
                } => ProjectTarget::Upgrade {
                    building_id: *target_instance_id,
                    building_type: BuildingTypeId::new(new_definition.id()),
                },
                ProjectResult::RecruitUnit { .. } => ProjectTarget::Other,
            },
            remaining_resources: positive(project.remaining_cost().needed_resources.iter()),
            remaining_turns: project.remaining_cost().turn_cost,
        })
        .collect()
}

fn market_offers(settlement: &Settlement) -> Vec<OfferSpec> {
    let mut offers: Vec<_> = settlement
        .market_offers()
        .iter()
        .map(|(&resource, offer)| match offer {
            TradeOffer::Sell(sell) => OfferSpec {
                resource,
                side: OfferSide::Sell,
                units: sell.get_amount_to_sell(),
                price_per_unit: sell.get_minimal_price_per_unit(),
            },
            TradeOffer::Buy(buy) => OfferSpec {
                resource,
                side: OfferSide::Buy,
                units: buy.get_remaining_amount_to_buy(),
                price_per_unit: buy.get_maximum_price_per_unit(),
            },
        })
        .collect();
    offers.sort_by_key(|offer| offer.resource);
    offers
}

fn last_turn_observation(report: &TurnReport) -> LastTurnObservation {
    let mut lost = std::collections::BTreeMap::new();
    for loss in &report.inventory.losses {
        *lost.entry(loss.resource).or_default() += loss.amount;
    }
    let ids = |completed: bool| {
        report
            .projects
            .iter()
            .filter(|outcome| outcome.result.is_ok() == completed)
            .map(|outcome| outcome.project_id)
            .collect()
    };
    LastTurnObservation {
        satisfaction_percent: report.consumption.satisfaction_percent,
        missing_needs: report.consumption.missing_resources.clone(),
        produced: report.inventory.produced.clone(),
        population_consumed: report.inventory.population_consumed.clone(),
        project_consumed: report.inventory.project_consumed.clone(),
        production_consumed: report.inventory.production_consumed.clone(),
        lost,
        trade_reserved: report.inventory.trade_reserved.clone(),
        trade_received: report.inventory.trade_received.clone(),
        gold_received: report.trade.gold_received,
        market_trades: report.trade.market_trades.len(),
        caravans_dispatched: report.trade.caravans_dispatched.len(),
        unfunded_caravan_orders: report.trade.unfunded_caravan_orders.len(),
        completed_projects: ids(true),
        failed_projects: ids(false),
    }
}
