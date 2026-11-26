//! Armies in the world (gameplay specification §10): recruitment from
//! settlement populations, disbanding, grouping, upkeep from the national
//! stock and treasury, morale and desertion.
//!
//! World owns these operations because they need several aggregates: the
//! settlement gives its people, the nation its equipment, food and gold, and
//! its decrees change the pool, the recruitment time and the upkeep.
use super::World;
use super::nations::NationOperationError;
use crate::domain::economy::{FoodClass, ResourceType};
use crate::domain::ids::{ArmyId, NationId, SettlementId};
use crate::domain::military::{
    Army, ArmyRegistry, ArmyUpkeepReport, DesertedUnit, Recruitment, RecruitsJoined, Unit,
    UnitTypeId,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A settlement's conscription pool (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conscription {
    /// Soldiers raised here who serve or are being recruited.
    pub serving: u32,
    /// Most soldiers that may serve at the same time: the pool share (10%,
    /// ×2 with Mobilization) of the population plus those serving.
    pub limit: u32,
    /// Soldiers that may still be recruited here now.
    pub available: u32,
}

/// What a recruitment order did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecruitmentOrder {
    pub soldiers: u32,
    /// Turns until the units join an army; 0 when they joined at once.
    pub ready_in_turns: u32,
    /// The army the units joined when they were ready at once.
    pub army_id: Option<ArmyId>,
}

/// Read access.
impl World {
    pub fn armies(&self) -> &ArmyRegistry {
        &self.armies
    }

    pub fn army(&self, id: ArmyId) -> Option<&Army> {
        self.armies.get(id)
    }

    /// The conscription pool of an existing settlement, for its owner's
    /// current decrees.
    pub fn conscription(&self, settlement_id: SettlementId) -> Option<Conscription> {
        let settlement = self.settlement(settlement_id)?;
        let owner = self.settlement_owner(settlement_id)?;
        let effects = self.nation_decree_effects(owner).ok()?;
        let share = (self.unit_catalog.rules().conscription_share
            * effects.conscription_pool_multiplier)
            .clamp(0.0, 1.0);
        let serving = self.armies.serving_from(settlement_id);
        let base = u64::from(settlement.population()) + u64::from(serving);
        // Rounded to 6 decimals first, so 0.1 × 1000 is 100, not 99.
        let share = (f64::from(share) * 1e6).round() / 1e6;
        let limit = u32::try_from((base as f64 * share + 1e-9).floor() as u64).unwrap_or(u32::MAX);
        Some(Conscription {
            serving,
            limit,
            available: limit.saturating_sub(serving),
        })
    }

    /// Provinces per turn: the speed of the army's slowest unit (§10.4);
    /// 0 for an army without units.
    pub fn army_movement(&self, army: &Army) -> u32 {
        army.units()
            .iter()
            .map(|unit| {
                self.unit_catalog
                    .get(&unit.unit_type)
                    .map_or(0, |definition| definition.movement())
            })
            .min()
            .unwrap_or(0)
    }

    /// Food and gold the nation's armies need per turn now.
    pub fn army_upkeep_demand(&self, nation_id: NationId) -> (u64, u64) {
        let catalog = &self.unit_catalog;
        let multiplier = self
            .nation_decree_effects(nation_id)
            .map_or(1.0, |effects| effects.army_upkeep_multiplier);
        let (mut food, mut gold) = (0_u64, 0_u64);
        for unit in self.armies.of_nation(nation_id).flat_map(Army::units) {
            let definition = catalog.get(&unit.unit_type);
            let per_food = definition.map_or(0, |definition| definition.food_upkeep());
            let per_gold = definition.map_or(0, |definition| definition.gold_upkeep());
            food += u64::from(unit.soldiers) * u64::from(per_food);
            gold += u64::from(unit.soldiers) * u64::from(per_gold);
        }
        // Discipline (§7): army upkeep ×1.25, rounded up.
        let gold = (gold as f64 * f64::from(multiplier)).ceil() as u64;
        (food, gold)
    }
}

/// Orders of a nation, for the nation decision handler. Each one checks
/// everything first and changes nothing when it fails.
impl World {
    fn owned_army(
        &self,
        nation_id: NationId,
        army_id: ArmyId,
    ) -> Result<&Army, NationOperationError> {
        let army = self
            .armies
            .get(army_id)
            .ok_or(NationOperationError::UnknownArmy(army_id))?;
        if army.nation() != nation_id {
            return Err(NationOperationError::ArmyNotOwned(army_id));
        }
        Ok(army)
    }

    /// Takes `units × soldiers_per_unit` people from an own settlement and the
    /// equipment from the national stock (§10.2). The units join an army in
    /// the settlement's province after the recruitment time, or at once with
    /// Mobilization.
    pub(crate) fn recruit_units(
        &mut self,
        nation_id: NationId,
        settlement_id: SettlementId,
        unit_type: &UnitTypeId,
        units: u32,
    ) -> Result<RecruitmentOrder, NationOperationError> {
        // 1. What is recruited.
        let definition = self
            .unit_catalog
            .get(unit_type)
            .ok_or_else(|| NationOperationError::UnknownUnitType(unit_type.clone()))?
            .clone();
        if units == 0 {
            return Err(NationOperationError::EmptyRecruitment);
        }
        // 2. Where from.
        if self.settlement(settlement_id).is_none() {
            return Err(NationOperationError::UnknownSettlement(settlement_id));
        }
        if self.settlement_owner(settlement_id) != Some(nation_id) {
            return Err(NationOperationError::SettlementNotOwned(settlement_id));
        }
        let rules = self.unit_catalog.rules().clone();
        let soldiers = units.saturating_mul(rules.soldiers_per_unit);
        let pool = self
            .conscription(settlement_id)
            .expect("an owned settlement has a pool");
        if soldiers > pool.available {
            return Err(NationOperationError::ConscriptionPoolExceeded {
                settlement_id,
                requested: soldiers,
                available: pool.available,
            });
        }
        // 3. The equipment in the national stock.
        let nation = self
            .nation(nation_id)
            .ok_or(NationOperationError::MissingNation(nation_id))?;
        let mut equipment = BTreeMap::new();
        for (resource, &per_unit) in definition.equipment() {
            let requested = per_unit.saturating_mul(units);
            let available = nation.stock().amount(resource.clone());
            if requested > available {
                return Err(NationOperationError::InsufficientStock {
                    resource: resource.clone(),
                    requested,
                    available,
                });
            }
            equipment.insert(resource.clone(), requested);
        }
        let instant = self
            .nation_decree_effects(nation_id)
            .is_ok_and(|effects| effects.instant_recruitment)
            || rules.recruitment_turns == 0;
        let province = self
            .settlement_province(settlement_id)
            .expect("a registered settlement has a province");
        if instant && !self.armies.can_station(nation_id, province) {
            // A new army would be needed and no ID is left: fail before
            // taking anything.
            return Err(NationOperationError::ArmyIdExhausted);
        }

        // 4. Checked: take the equipment and the people.
        let nation = self.nation_mut(nation_id).expect("checked above");
        for (resource, amount) in equipment {
            assert!(nation.withdraw_resource(resource, amount));
        }
        self.settlement_mut(settlement_id)
            .expect("checked above")
            .remove_population(soldiers);
        if instant {
            let unit = Unit {
                unit_type: unit_type.clone(),
                soldiers: rules.soldiers_per_unit,
                morale: rules.initial_morale,
                raised_in: settlement_id,
            };
            let army_id = self
                .armies
                .station(nation_id, province, vec![unit; units as usize])
                .expect("an ID was checked above");
            return Ok(RecruitmentOrder {
                soldiers,
                ready_in_turns: 0,
                army_id: Some(army_id),
            });
        }
        self.armies.add_recruitment(Recruitment {
            nation: nation_id,
            settlement: settlement_id,
            unit_type: unit_type.clone(),
            units,
            soldiers_per_unit: rules.soldiers_per_unit,
            turns_left: rules.recruitment_turns,
        });
        Ok(RecruitmentOrder {
            soldiers,
            ready_in_turns: rules.recruitment_turns,
            army_id: None,
        })
    }

    /// Disbands an own army standing in an own province: its living soldiers
    /// join that province's settlement (not the one they came from). Returns
    /// the settlement and the soldiers.
    pub(crate) fn disband_army(
        &mut self,
        nation_id: NationId,
        army_id: ArmyId,
    ) -> Result<(SettlementId, u32), NationOperationError> {
        let army = self.owned_army(nation_id, army_id)?;
        let province = army.province();
        let soldiers = army.soldiers();
        let settlement_id = self
            .province_settlement(province)
            .ok()
            .flatten()
            .filter(|&settlement| self.settlement_owner(settlement) == Some(nation_id))
            .ok_or(NationOperationError::NotOwnProvince { army_id, province })?;
        self.armies.remove(army_id);
        self.settlement_mut(settlement_id)
            .expect("checked above")
            .add_population(soldiers);
        Ok((settlement_id, soldiers))
    }

    /// Moves every unit of `army_id` into `into`; both are own armies in the
    /// same province. `army_id` disappears.
    pub(crate) fn merge_armies(
        &mut self,
        nation_id: NationId,
        army_id: ArmyId,
        into: ArmyId,
    ) -> Result<(), NationOperationError> {
        if army_id == into {
            return Err(NationOperationError::SameArmy(army_id));
        }
        let province = self.owned_army(nation_id, army_id)?.province();
        if self.owned_army(nation_id, into)?.province() != province {
            return Err(NationOperationError::ArmiesApart { army_id, into });
        }
        let units = self
            .armies
            .remove(army_id)
            .expect("checked above")
            .units()
            .to_vec();
        self.armies
            .get_mut(into)
            .expect("checked above")
            .units_mut()
            .extend(units);
        Ok(())
    }

    /// Moves the units at `unit_indices` (positions in the army's unit list)
    /// into a new army in the same province; at least one unit stays.
    /// Returns the new army.
    pub(crate) fn split_army(
        &mut self,
        nation_id: NationId,
        army_id: ArmyId,
        unit_indices: &[usize],
    ) -> Result<ArmyId, NationOperationError> {
        let army = self.owned_army(nation_id, army_id)?;
        let selected: BTreeSet<_> = unit_indices.iter().copied().collect();
        if selected.is_empty()
            || selected.len() != unit_indices.len()
            || selected.len() >= army.units().len()
            || selected.iter().any(|&index| index >= army.units().len())
        {
            return Err(NationOperationError::InvalidUnitSelection(army_id));
        }
        let province = army.province();
        let new_id = self
            .armies
            .create(nation_id, province)
            .ok_or(NationOperationError::ArmyIdExhausted)?;
        let units = std::mem::take(
            self.armies
                .get_mut(army_id)
                .expect("checked above")
                .units_mut(),
        );
        let (moved, kept): (Vec<_>, Vec<_>) = units
            .into_iter()
            .enumerate()
            .partition(|(index, _)| selected.contains(index));
        *self
            .armies
            .get_mut(army_id)
            .expect("checked above")
            .units_mut() = kept.into_iter().map(|(_, unit)| unit).collect();
        *self
            .armies
            .get_mut(new_id)
            .expect("created above")
            .units_mut() = moved.into_iter().map(|(_, unit)| unit).collect();
        Ok(new_id)
    }
}

/// The turn's military bookkeeping.
impl World {
    /// Every turn, right after the decree upkeep: each nation feeds and pays
    /// its armies from the national stock and treasury (§10.3). What is
    /// available is shared equally: every unit gets the same share, so the
    /// morale of all units changes alike: +20 with full upkeep (+30 when all
    /// the food was tier 2), otherwise −20 (×0.5 with Discipline). Units at
    /// morale 0 desert and are gone.
    pub(crate) fn pay_army_upkeep(&mut self) -> BTreeMap<NationId, ArmyUpkeepReport> {
        let nations: Vec<_> = self.nations.iter().map(|nation| nation.id()).collect();
        let rules = self.unit_catalog.rules().clone();
        // Tier-2 (processed) food first, then tier 1, in catalogue order;
        // `true` marks tier 2.
        let catalog = std::sync::Arc::clone(&self.resource_catalog);
        let food_class =
            |resource: &ResourceType| catalog.get(resource).and_then(|d| d.food_class());
        let foods: Vec<(ResourceType, bool)> = [FoodClass::Processed, FoodClass::Raw]
            .into_iter()
            .flat_map(|class| {
                catalog
                    .materials()
                    .iter()
                    .filter(move |resource| food_class(resource) == Some(class))
                    .map(move |resource| (resource.clone(), class == FoodClass::Processed))
            })
            .collect();
        let mut reports = BTreeMap::new();
        for nation_id in nations {
            let soldiers: u32 = self.armies.of_nation(nation_id).map(Army::soldiers).sum();
            if soldiers == 0 {
                continue;
            }
            let (food_demand, gold_demand) = self.army_upkeep_demand(nation_id);
            let loss_multiplier = self
                .nation_decree_effects(nation_id)
                .map_or(1.0, |effects| effects.morale_loss_multiplier);
            let nation = self.nation_mut(nation_id).expect("listed above");
            let mut report = ArmyUpkeepReport {
                soldiers,
                food_demand,
                gold_demand,
                ..ArmyUpkeepReport::default()
            };
            for (food, tier2) in &foods {
                let missing = food_demand - report.food_paid;
                if missing == 0 {
                    break;
                }
                let stored = u64::from(nation.stock().amount(food.clone()));
                let taken = stored.min(missing);
                if taken > 0 {
                    let amount = u32::try_from(taken).expect("at most a stored u32 amount");
                    assert!(nation.withdraw_resource(food.clone(), amount));
                    report.food_paid += taken;
                    if *tier2 {
                        report.tier2_food_paid += taken;
                    }
                }
            }
            report.gold_paid = gold_demand.min(nation.treasury());
            assert!(nation.withdraw_gold(report.gold_paid));
            report.fully_supplied =
                report.food_paid == food_demand && report.gold_paid == gold_demand;
            report.morale_change = if report.fully_supplied {
                if food_demand > 0 && report.tier2_food_paid == food_demand {
                    rules.morale_gain_tier2_food as i32
                } else {
                    rules.morale_gain_full_upkeep as i32
                }
            } else {
                -((rules.morale_loss_partial_upkeep as f32 * loss_multiplier).round() as i32)
            };
            let army_ids: Vec<_> = self.armies.of_nation(nation_id).map(Army::id).collect();
            for army_id in army_ids {
                let army = self.armies.get_mut(army_id).expect("listed above");
                let units = std::mem::take(army.units_mut());
                for mut unit in units {
                    unit.morale = (i64::from(unit.morale) + i64::from(report.morale_change))
                        .clamp(0, i64::from(rules.max_morale))
                        as u32;
                    if unit.morale == 0 {
                        report.deserted.push(DesertedUnit {
                            army_id,
                            unit_type: unit.unit_type,
                            soldiers: unit.soldiers,
                        });
                    } else {
                        army.units_mut().push(unit);
                    }
                }
            }
            reports.insert(nation_id, report);
        }
        self.armies.remove_empty();
        reports
    }

    /// End of a turn: recruitments count down and finished units join an
    /// army in their settlement's province, or at the nation's capital when
    /// the settlement was lost meanwhile.
    pub(crate) fn finish_recruitments(&mut self) -> BTreeMap<NationId, Vec<RecruitsJoined>> {
        let initial_morale = self.unit_catalog.rules().initial_morale;
        let mut joined: BTreeMap<NationId, Vec<RecruitsJoined>> = BTreeMap::new();
        for recruitment in self.armies.advance_recruitments() {
            let Some(nation) = self.nation(recruitment.nation) else {
                continue;
            };
            let home = if self.settlement_owner(recruitment.settlement) == Some(nation.id()) {
                recruitment.settlement
            } else {
                nation.capital()
            };
            let Some(province) = self.settlement_province(home) else {
                continue;
            };
            let unit = Unit {
                unit_type: recruitment.unit_type.clone(),
                soldiers: recruitment.soldiers_per_unit,
                morale: initial_morale,
                raised_in: recruitment.settlement,
            };
            let Some(army_id) = self.armies.station(
                recruitment.nation,
                province,
                vec![unit; recruitment.units as usize],
            ) else {
                continue;
            };
            joined
                .entry(recruitment.nation)
                .or_default()
                .push(RecruitsJoined {
                    army_id,
                    settlement_id: recruitment.settlement,
                    unit_type: recruitment.unit_type,
                    units: recruitment.units,
                });
        }
        joined
    }
}
