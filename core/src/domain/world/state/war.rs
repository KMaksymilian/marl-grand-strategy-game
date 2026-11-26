//! War on the map (gameplay specification §11): access to provinces, marches
//! from province to province, sieges, battles for settlements with garrisons
//! and walls, retreats and conquest.
//!
//! An army occupies a whole province. Every turn, after the armies are fed:
//!
//! 1. armies standing where they may no longer be (after a peace or a broken
//!    alliance) return to the nearest own province;
//! 2. armies march towards their destination, one province per movement point,
//!    and stop when they enter an enemy province: that is an attack on its
//!    settlement;
//! 3. one battle in every province where an army of a nation at war with the
//!    settlement's owner stands: the attackers against the garrison and the
//!    armies of the owner and its allies there;
//! 4. settlements with an enemy army in their province are under attack: no
//!    tax, no trade (and, for the economy, half production).
//!
//! At the end of the turn, garrisons of settlements not under attack refill.
use super::World;
use super::nations::NationOperationError;
use crate::domain::ids::{ArmyId, NationId, SettlementId};
use crate::domain::military::battle::{self, Stat};
use crate::domain::military::{
    Army, ArmyMarch, ArmyWithdrawal, BattleReport, BattleSide, Garrison, MarchOutcome, Unit,
};
use crate::domain::nation::diplomacy::Relation;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::collections::{BTreeMap, BTreeSet};

/// What the war part of a turn did, per nation, for the nation reports.
#[derive(Debug, Default)]
pub(crate) struct WarTurn {
    pub withdrawals: BTreeMap<NationId, Vec<ArmyWithdrawal>>,
    pub marches: BTreeMap<NationId, Vec<ArmyMarch>>,
    pub battles: BTreeMap<NationId, Vec<BattleReport>>,
}

/// Access to provinces and the inputs of a battle.
impl World {
    /// Seeds the random variance of battles; the same seed and decisions
    /// always give the same battles. The default seed is 0.
    pub fn set_battle_seed(&mut self, seed: u64) {
        self.battle_rng = StdRng::seed_from_u64(seed);
    }

    fn relation_between(&self, a: NationId, b: NationId) -> Option<Relation> {
        (a != b).then(|| self.diplomacy.relation(a, b))
    }

    fn at_war(&self, a: NationId, b: NationId) -> bool {
        matches!(self.relation_between(a, b), Some(Relation::War { .. }))
    }

    /// Whether the nation's armies may enter the province (§11.1): its own,
    /// an ally's or one of a nation at war with it. A province without a
    /// settlement belongs to nobody and may be crossed by everyone.
    pub fn may_enter(&self, nation: NationId, province: usize) -> bool {
        match self.province_owner(province) {
            Ok(None) => true,
            Ok(Some(owner)) => {
                owner == nation
                    || matches!(
                        self.relation_between(nation, owner),
                        Some(Relation::Alliance | Relation::War { .. })
                    )
            }
            Err(_) => false,
        }
    }

    /// The province belongs to a nation at war with this one.
    pub fn is_enemy_province(&self, nation: NationId, province: usize) -> bool {
        self.province_owner(province)
            .ok()
            .flatten()
            .is_some_and(|owner| self.at_war(nation, owner))
    }

    /// Terrain multiplier of a battle in the province (§11.3); 1.0 until the
    /// terrain rules exist.
    pub fn terrain_modifier(&self, _province: usize) -> f32 {
        1.0
    }

    /// Bonus of the walls to the defence in the province (0.25, 0.5 or 1.0
    /// for the wall levels, §11.4), plus the owner's decree points.
    pub fn wall_bonus(&self, province: usize, owner: NationId) -> f32 {
        // ECONOMY (walls, §11.4): walls are a building of the economy. Take
        // the level's bonus here, e.g. from
        // `province_fortification_defense_basis_points(province) / 10_000`.
        let _ = province;
        let walls = 0.0;
        let decree_points = self
            .nation_decree_effects(owner)
            .map_or(0.0, |effects| effects.wall_bonus_points);
        walls + decree_points / 100.0
    }

    /// The settlement's garrison now: after battles, or full (levy from
    /// `garrison_share` of the people, full morale).
    pub fn garrison(&self, settlement_id: SettlementId) -> Option<Garrison> {
        let settlement = self.settlement(settlement_id)?;
        Some(self.armies.garrison(settlement_id).unwrap_or_else(|| {
            let rules = self.unit_catalog.rules();
            let share = f64::from(rules.garrison_share);
            Garrison {
                soldiers: (f64::from(settlement.population()) * share + 1e-9).floor() as u32,
                morale: rules.initial_morale,
            }
        }))
    }
}

/// The march order of a nation, for the nation decision handler.
impl World {
    /// Sets the army's destination; it marches from this turn's movement on,
    /// along a route recomputed every turn. Fails when no allowed route
    /// exists now. Returns the planned route (empty: the army stays).
    pub(crate) fn order_army_march(
        &mut self,
        nation_id: NationId,
        army_id: ArmyId,
        province: usize,
    ) -> Result<Vec<usize>, NationOperationError> {
        let from = self.owned_army(nation_id, army_id)?.province();
        if self.province(province).is_err() {
            return Err(NationOperationError::UnknownProvince(province));
        }
        let route = self
            .province_graph
            .route(from, province, |next| self.may_enter(nation_id, next))
            .ok_or(NationOperationError::NoRoute { army_id, province })?;
        let destination = (province != from).then_some(province);
        self.armies
            .get_mut(army_id)
            .expect("checked above")
            .set_destination(destination);
        Ok(route)
    }
}

/// The war part of a turn.
impl World {
    /// Withdrawals, marches and battles of this turn, then the sieges.
    pub(crate) fn resolve_war(&mut self) -> WarTurn {
        let mut turn = WarTurn::default();
        self.withdraw_displaced_armies(&mut turn);
        self.march_armies(&mut turn);
        self.fight_battles(&mut turn);
        self.refresh_sieges();
        turn
    }

    /// Armies standing where they may no longer be return to the nearest own
    /// province; without one they are disbanded and their soldiers lost.
    // §8.5: after a peace, armies on the other side's territory go home.
    fn withdraw_displaced_armies(&mut self, turn: &mut WarTurn) {
        let ids: Vec<_> = self.armies.iter().map(Army::id).collect();
        for army_id in ids {
            let army = self.armies.get(army_id).expect("listed above");
            let (nation, from) = (army.nation(), army.province());
            if self.may_enter(nation, from) {
                continue;
            }
            let route = self.province_graph.route_to_nearest(
                from,
                |next| self.may_enter(nation, next),
                |next| self.province_owner(next).ok().flatten() == Some(nation),
            );
            let to = route.and_then(|route| route.last().copied());
            match to {
                Some(home) => self.armies.get_mut(army_id).expect("listed").relocate(home),
                None => {
                    self.armies.remove(army_id);
                }
            }
            turn.withdrawals
                .entry(nation)
                .or_default()
                .push(ArmyWithdrawal { army_id, from, to });
        }
    }

    /// Every army with a destination enters one province per movement point
    /// along the shortest allowed route, recomputed at every step; it stops
    /// when it arrives, when it enters an enemy province and when no route is
    /// left. Ascending army ID.
    fn march_armies(&mut self, turn: &mut WarTurn) {
        let ids: Vec<_> = self
            .armies
            .iter()
            .filter(|army| army.destination().is_some())
            .map(Army::id)
            .collect();
        for army_id in ids {
            let army = self.armies.get(army_id).expect("listed above");
            let (nation, destination) = (army.nation(), army.destination().expect("filtered"));
            let speed = self.army_movement(army);
            let mut path = Vec::new();
            let mut outcome = MarchOutcome::Marching;
            for _ in 0..speed {
                let current = self.armies.get(army_id).expect("listed").province();
                if current == destination {
                    outcome = MarchOutcome::Arrived;
                    break;
                }
                let Some(route) = self
                    .province_graph
                    .route(current, destination, |next| self.may_enter(nation, next))
                else {
                    outcome = MarchOutcome::NoRoute;
                    break;
                };
                let next = route[0];
                self.armies.get_mut(army_id).expect("listed").enter(next);
                path.push(next);
                if self.is_enemy_province(nation, next) {
                    outcome = MarchOutcome::Attacking;
                    break;
                }
                if next == destination {
                    outcome = MarchOutcome::Arrived;
                    break;
                }
            }
            if outcome != MarchOutcome::Marching {
                self.armies
                    .get_mut(army_id)
                    .expect("listed")
                    .set_destination(None);
            }
            turn.marches.entry(nation).or_default().push(ArmyMarch {
                army_id,
                path,
                outcome,
            });
        }
    }

    /// One battle in every province (ascending ID) where an army of a nation
    /// at war with the settlement's owner stands.
    fn fight_battles(&mut self, turn: &mut WarTurn) {
        let provinces: BTreeSet<_> = self.armies.iter().map(Army::province).collect();
        for province in provinces {
            let Some(settlement_id) = self.province_settlement(province).ok().flatten() else {
                continue;
            };
            let Some(owner) = self.settlement_owner(settlement_id) else {
                continue;
            };
            let attackers: Vec<_> = self
                .armies
                .iter()
                .filter(|army| army.province() == province && self.at_war(army.nation(), owner))
                .map(Army::id)
                .collect();
            if attackers.is_empty() {
                continue;
            }
            let defenders: Vec<_> = self
                .armies
                .iter()
                .filter(|army| {
                    army.province() == province
                        && (army.nation() == owner
                            || self.relation_between(army.nation(), owner)
                                == Some(Relation::Alliance))
                })
                .map(Army::id)
                .collect();
            let report = self.fight(province, settlement_id, owner, &attackers, &defenders);
            let mut involved: BTreeSet<NationId> = report.attackers.iter().copied().collect();
            involved.insert(owner);
            involved.extend(
                defenders
                    .iter()
                    .filter_map(|&id| self.armies.get(id).map(Army::nation)),
            );
            for nation in involved {
                turn.battles.entry(nation).or_default().push(report.clone());
            }
        }
    }

    fn units_of(&self, armies: &[ArmyId]) -> Vec<Unit> {
        armies
            .iter()
            .filter_map(|&id| self.armies.get(id))
            .flat_map(|army| army.units().iter().cloned())
            .collect()
    }

    fn morale_loss_multiplier(&self, nation: NationId) -> f32 {
        self.nation_decree_effects(nation)
            .map_or(1.0, |effects| effects.morale_loss_multiplier)
    }

    /// One battle for a settlement (§11.3). Returns its report.
    fn fight(
        &mut self,
        province: usize,
        settlement_id: SettlementId,
        owner: NationId,
        attackers: &[ArmyId],
        defenders: &[ArmyId],
    ) -> BattleReport {
        let rules = self.unit_catalog.rules().clone();
        let garrison = self
            .garrison(settlement_id)
            .expect("an existing settlement");
        let garrison_unit = Unit {
            unit_type: rules.garrison_unit.clone(),
            soldiers: garrison.soldiers,
            morale: garrison.morale,
            raised_in: settlement_id,
        };
        let attacking_units = self.units_of(attackers);
        let mut defending_units = self.units_of(defenders);
        if garrison.soldiers > 0 {
            defending_units.push(garrison_unit.clone());
        }
        let mut report = BattleReport {
            province_id: province,
            settlement_id,
            defender: owner,
            attackers: attackers
                .iter()
                .filter_map(|&id| self.armies.get(id).map(Army::nation))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            attacking_armies: attackers.to_vec(),
            defending_armies: defenders.to_vec(),
            garrison_soldiers: garrison.soldiers,
            wall_multiplier: 1.0,
            attacker_strength: 0.0,
            defender_strength: 0.0,
            winner: BattleSide::Attackers,
            attacker_losses: 0,
            defender_losses: 0,
            retreated: Vec::new(),
            destroyed: Vec::new(),
            conquered_by: None,
        };

        // §11.5: a settlement without defenders falls without a battle.
        if defending_units.iter().all(|unit| unit.soldiers == 0) {
            self.conquer(settlement_id, defenders, &mut report);
            return report;
        }

        let catalog = std::sync::Arc::clone(&self.unit_catalog);
        let terrain = self.terrain_modifier(province);
        let walls =
            battle::wall_multiplier(&catalog, self.wall_bonus(province, owner), &attacking_units);
        let variance = rules.strength_variance;
        let mut roll = || {
            self.battle_rng
                .random_range(1.0 - variance..=1.0 + variance)
        };
        let attack = battle::side_strength(
            &catalog,
            &attacking_units,
            &defending_units,
            Stat::Attack,
            terrain,
        ) * roll();
        let defense = battle::side_strength(
            &catalog,
            &defending_units,
            &attacking_units,
            Stat::Defense,
            terrain,
        ) * walls
            * roll();
        report.wall_multiplier = walls;
        report.attacker_strength = attack;
        report.defender_strength = defense;
        // A tie goes to the defenders.
        let attackers_win = attack > defense;
        report.winner = if attackers_win {
            BattleSide::Attackers
        } else {
            BattleSide::Defenders
        };
        let (winner_strength, loser_strength) = if attackers_win {
            (attack, defense)
        } else {
            (defense, attack)
        };
        let winner_share = if winner_strength > 0.0 {
            rules.loser_unit_losses * loser_strength / winner_strength
        } else {
            0.0
        };
        let share = |side_won: bool| {
            if side_won {
                winner_share
            } else {
                rules.loser_unit_losses
            }
        };

        // Losses: every unit of the loser loses 30% (rounded up) and morale;
        // every unit of the winner loses 30% × the strength ratio.
        for &army_id in attackers {
            report.attacker_losses += self.take_losses(
                army_id,
                share(attackers_win),
                !attackers_win,
                rules.loser_morale_loss,
            );
        }
        for &army_id in defenders {
            report.defender_losses += self.take_losses(
                army_id,
                share(!attackers_win),
                attackers_win,
                rules.loser_morale_loss,
            );
        }
        let mut garrison_after = garrison_unit;
        if garrison.soldiers > 0 {
            let fallen =
                battle::lose_soldiers(&mut garrison_after, share(!attackers_win), attackers_win);
            if attackers_win {
                let loss = (rules.loser_morale_loss as f32 * self.morale_loss_multiplier(owner))
                    .round() as u32;
                garrison_after.morale = garrison_after.morale.saturating_sub(loss);
            }
            report.defender_losses += fallen;
            // §11.4: the fallen of the garrison leave the population.
            self.settlement_mut(settlement_id)
                .expect("an existing settlement")
                .remove_population(fallen);
            self.armies.set_garrison(
                settlement_id,
                Garrison {
                    soldiers: garrison_after.soldiers,
                    morale: garrison_after.morale,
                },
            );
        }

        // Armies without soldiers are gone; attackers at morale 0 desert.
        for &army_id in attackers.iter().chain(defenders) {
            let Some(army) = self.armies.get_mut(army_id) else {
                continue;
            };
            let attacking = attackers.contains(&army_id);
            army.units_mut()
                .retain(|unit| unit.soldiers > 0 && (!attacking || unit.morale > 0));
            if army.units().is_empty() {
                self.armies.remove(army_id);
                report.destroyed.push(army_id);
            }
        }

        if attackers_win {
            // §11.5: the settlement falls when its defenders have no units
            // left or all their morale is gone.
            let standing = defenders
                .iter()
                .filter_map(|&id| self.armies.get(id))
                .flat_map(Army::units)
                .any(|unit| unit.soldiers > 0 && unit.morale > 0)
                || (garrison_after.soldiers > 0 && garrison_after.morale > 0);
            if !standing {
                self.conquer(settlement_id, defenders, &mut report);
            }
        } else {
            // §11.3: a beaten attacker retreats to where it came from, if it
            // may stand there; otherwise it is destroyed. The attack ends.
            for &army_id in attackers {
                let Some(army) = self.armies.get(army_id) else {
                    continue;
                };
                let nation = army.nation();
                let back = army.came_from().filter(|&previous| {
                    self.may_enter(nation, previous) && !self.is_enemy_province(nation, previous)
                });
                match back {
                    Some(previous) => {
                        self.armies
                            .get_mut(army_id)
                            .expect("exists")
                            .relocate(previous);
                        report.retreated.push(army_id);
                    }
                    None => {
                        self.armies.remove(army_id);
                        report.destroyed.push(army_id);
                    }
                }
            }
        }
        report
    }

    /// Applies an army's share of losses; the loser also loses morale.
    /// Returns the fallen soldiers.
    fn take_losses(&mut self, army_id: ArmyId, share: f32, lost: bool, morale_loss: u32) -> u32 {
        let Some(nation) = self.armies.get(army_id).map(Army::nation) else {
            return 0;
        };
        let morale_loss = if lost {
            (morale_loss as f32 * self.morale_loss_multiplier(nation)).round() as u32
        } else {
            0
        };
        let army = self.armies.get_mut(army_id).expect("checked above");
        let mut fallen = 0;
        for unit in army.units_mut() {
            fallen += battle::lose_soldiers(unit, share, lost);
            unit.morale = unit.morale.saturating_sub(morale_loss);
        }
        fallen
    }

    /// The settlement and its province pass to the attacking nation with the
    /// most soldiers there (lower ID on a tie); the defending armies there
    /// are destroyed (§11.5). Ownership change, conquest penalty, half tax and
    /// capital loss follow from `transfer_settlement`.
    fn conquer(
        &mut self,
        settlement_id: SettlementId,
        defenders: &[ArmyId],
        report: &mut BattleReport,
    ) {
        let province = report.province_id;
        let mut soldiers: BTreeMap<NationId, u32> = BTreeMap::new();
        for &army_id in &report.attacking_armies {
            if let Some(army) = self
                .armies
                .get(army_id)
                .filter(|army| army.province() == province)
            {
                *soldiers.entry(army.nation()).or_default() += army.soldiers();
            }
        }
        let Some(conqueror) = soldiers
            .iter()
            .max_by_key(|&(&nation, &count)| (count, std::cmp::Reverse(nation)))
            .map(|(&nation, _)| nation)
        else {
            return;
        };
        for &army_id in defenders {
            if self.armies.remove(army_id).is_some() {
                report.destroyed.push(army_id);
            }
        }
        if self.transfer_settlement(settlement_id, conqueror).is_ok() {
            report.conquered_by = Some(conqueror);
            // The garrison refills for the new owner once no enemy stands here.
            let morale = self.unit_catalog.rules().initial_morale;
            self.armies.set_garrison(
                settlement_id,
                Garrison {
                    soldiers: 0,
                    morale,
                },
            );
            // ECONOMY (walls, §11.5): the walls of a conquered settlement lose
            // one level; call the economy's downgrade here.
        }
    }

    /// A settlement is under attack while an army of a nation at war with its
    /// owner stands in its province (§11.2). Only sieges by armies are set
    /// and cleared here; a scenario's own flag stays.
    fn refresh_sieges(&mut self) {
        let now: BTreeSet<SettlementId> = self
            .armies
            .iter()
            .filter_map(|army| {
                let settlement = self.province_settlement(army.province()).ok().flatten()?;
                let owner = self.settlement_owner(settlement)?;
                self.at_war(army.nation(), owner).then_some(settlement)
            })
            .collect();
        for &ended in self.sieges.difference(&now) {
            self.nations.set_under_attack(ended, false);
        }
        for &settlement in &now {
            self.nations.set_under_attack(settlement, true);
        }
        self.sieges = now;
    }

    /// End of a turn: garrisons of settlements not under attack refill to
    /// their share of the people (§11.4).
    pub(crate) fn refill_garrisons(&mut self) {
        let calm: Vec<_> = self
            .settlement_ids()
            .into_iter()
            .filter(|&id| {
                !self
                    .nations
                    .ownership(id)
                    .is_some_and(|ownership| ownership.is_under_attack())
            })
            .collect();
        for settlement in calm {
            self.armies.reset_garrison(settlement);
        }
    }
}
