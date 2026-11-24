//! Decrees (gameplay specification §7): nation-wide policies with an upkeep
//! paid every turn, a delayed start, a minimum duration and a cooldown.
//!
//! Which decrees exist, what they cost and what they do is content, loaded
//! from `decrees.toml` into a [`DecreeCatalog`](super::decree_catalog::DecreeCatalog).
//! This module holds what does not depend on the content: the identifier, the
//! effect data and one nation's decree lifecycle.
//!
//! Lifecycle of one decree enacted in turn T:
//!
//! | When | State | Effects | Upkeep |
//! | --- | --- | --- | --- |
//! | turn T | `Pending` | no | no |
//! | from turn T+1 | `Active` | yes | every turn |
//! | revoked by the nation after the minimum duration, or automatically when the upkeep cannot be paid | `Cooldown` | no | no |
//!
//! Pending and active decrees use slots; a decree on cooldown does not.
//!
//! The effects are only data (`DecreeEffects`): every system that a decree
//! changes reads the combined effects of the owner's active decrees
//! (`World::decree_effects`) and applies them itself.
use crate::domain::economy::{ResourceCatalog, ResourceCategory, ResourceDefinition, ResourceType};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Identifier of a decree, as written in `decrees.toml` and in agent JSON:
/// snake case (`"agricultural_decree"`). Whether a decree with this ID exists
/// is checked against the catalogue when an action is validated.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DecreeId(Arc<str>);

impl DecreeId {
    /// Lower-case words of letters and digits joined by single underscores,
    /// starting with a letter.
    pub fn new(id: &str) -> Option<Self> {
        let valid = id.split('_').all(|word| {
            word.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && word
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        });
        valid.then(|| Self(Arc::from(id)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for DecreeId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl std::fmt::Display for DecreeId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for DecreeId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DecreeId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let id = String::deserialize(deserializer)?;
        Self::new(&id).ok_or_else(|| serde::de::Error::custom(format!("invalid decree ID {id:?}")))
    }
}

/// A change of output for the materials that match both filters; an empty
/// filter matches every material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionChange {
    /// Resource categories of `resources.toml`; empty: every category.
    #[serde(default)]
    pub categories: Vec<ResourceCategory>,
    /// Material tiers (1–3); empty: every tier.
    #[serde(default)]
    pub tiers: Vec<u8>,
    /// Fraction of the output (+0.2 = +20%).
    pub change: f32,
}

impl ProductionChange {
    pub fn applies_to(&self, material: &ResourceDefinition) -> bool {
        (self.categories.is_empty() || self.categories.contains(&material.category()))
            && (self.tiers.is_empty() || self.tiers.contains(&material.tier()))
    }
}

/// Effects of one decree, or the combined effects of several. Fractions add up
/// (+0.2 and −0.1 give +0.1), multipliers multiply, flags combine with "or".
/// No decree active: every fraction is 0, every multiplier 1, every flag false.
/// In `decrees.toml` a missing field has this neutral value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DecreeEffects {
    // ECONOMY (production, §2 and §7): changes of output, applied by the
    // production system; `production_change` sums them for one material.
    pub production: Vec<ProductionChange>,
    // ECONOMY (construction): change of the material cost of new
    // constructions and upgrades, as a fraction (−0.2).
    pub construction_cost: f32,
    // ECONOMY (trade): change of caravan travel speed, as a fraction (+0.25).
    pub caravan_speed: f32,
    // ECONOMY (happiness, §4.2): happiness points in every settlement.
    pub happiness: f32,
    // ECONOMY (happiness, §4.2): multiplier of the monuments' happiness.
    pub monument_happiness_multiplier: f32,
    // ECONOMY (population needs, §4.1): change of food consumption, as a
    // fraction (−0.25).
    pub food_consumption: f32,
    /// Customs (§9): multiplier of the nation's customs duties.
    pub tariff_multiplier: f32,
    /// Taxation (§6): multiplier of the production tax on planks, steel,
    /// bronze and cattle; applies once those rules exist.
    pub military_levy_multiplier: f32,
    /// Army (§10.2): multiplier of the conscription pool.
    pub conscription_pool_multiplier: f32,
    /// Army (§10.2): recruitment takes no time.
    pub instant_recruitment: bool,
    /// Army (§10.3): multiplier of morale losses.
    pub morale_loss_multiplier: f32,
    /// Army (§10.3): multiplier of army upkeep.
    pub army_upkeep_multiplier: f32,
    /// Walls (§11.4): percentage points added to the wall bonus.
    pub wall_bonus_points: f32,
    /// Walls (§3.4): change of the material cost of walls, as a fraction (−0.3).
    pub wall_cost: f32,
}

impl Default for DecreeEffects {
    fn default() -> Self {
        Self {
            production: Vec::new(),
            construction_cost: 0.0,
            caravan_speed: 0.0,
            happiness: 0.0,
            monument_happiness_multiplier: 1.0,
            food_consumption: 0.0,
            tariff_multiplier: 1.0,
            military_levy_multiplier: 1.0,
            conscription_pool_multiplier: 1.0,
            instant_recruitment: false,
            morale_loss_multiplier: 1.0,
            army_upkeep_multiplier: 1.0,
            wall_bonus_points: 0.0,
            wall_cost: 0.0,
        }
    }
}

impl DecreeEffects {
    pub(crate) fn food_multiplier_basis_points(&self) -> u32 {
        multiplier_basis_points(self.food_consumption)
    }

    /// Resolves output filters against the shared material catalog once per turn.
    pub(crate) fn production_multipliers(
        &self,
        catalog: &ResourceCatalog,
    ) -> HashMap<ResourceType, u32> {
        if self.production.is_empty() {
            return HashMap::new();
        }
        catalog
            .materials()
            .iter()
            .filter_map(|id| {
                let material = catalog.get(id).expect("listed material exists");
                let multiplier = multiplier_basis_points(self.production_change(material));
                (multiplier != 10_000).then(|| (id.clone(), multiplier))
            })
            .collect()
    }

    /// Total change of output of one material, as a fraction: the sum of every
    /// production change that applies to it (0.0 when none does).
    pub fn production_change(&self, material: &ResourceDefinition) -> f32 {
        self.production
            .iter()
            .filter(|change| change.applies_to(material))
            .map(|change| change.change)
            .sum()
    }

    /// Adds another decree's effects to these.
    pub(crate) fn combine(&mut self, other: &Self) {
        self.production.extend(other.production.iter().cloned());
        self.construction_cost += other.construction_cost;
        self.caravan_speed += other.caravan_speed;
        self.happiness += other.happiness;
        self.monument_happiness_multiplier *= other.monument_happiness_multiplier;
        self.food_consumption += other.food_consumption;
        self.tariff_multiplier *= other.tariff_multiplier;
        self.military_levy_multiplier *= other.military_levy_multiplier;
        self.conscription_pool_multiplier *= other.conscription_pool_multiplier;
        self.instant_recruitment |= other.instant_recruitment;
        self.morale_loss_multiplier *= other.morale_loss_multiplier;
        self.army_upkeep_multiplier *= other.army_upkeep_multiplier;
        self.wall_bonus_points += other.wall_bonus_points;
        self.wall_cost += other.wall_cost;
    }

    /// Every number of the effects, for the catalogue's finiteness check.
    pub(crate) fn numbers(&self) -> impl Iterator<Item = f32> + '_ {
        [
            self.construction_cost,
            self.caravan_speed,
            self.happiness,
            self.monument_happiness_multiplier,
            self.food_consumption,
            self.tariff_multiplier,
            self.military_levy_multiplier,
            self.conscription_pool_multiplier,
            self.morale_loss_multiplier,
            self.army_upkeep_multiplier,
            self.wall_bonus_points,
            self.wall_cost,
        ]
        .into_iter()
        .chain(self.production.iter().map(|change| change.change))
    }
}

fn multiplier_basis_points(change: f32) -> u32 {
    (10_000.0 + f64::from(change) * 10_000.0).round().max(0.0) as u32
}

/// Where one decree is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum DecreeState {
    /// Enacted this turn; active from the next turn.
    Pending,
    /// In effect; `turns_active` counts completed active turns.
    Active { turns_active: u32 },
    /// Revoked (by the nation, or automatically for unpaid upkeep); can be
    /// enacted again when `turns_left` reaches zero.
    Cooldown { turns_left: u32 },
}

/// Upkeep of one nation's decrees in one turn, for the nation's report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecreeUpkeepReport {
    /// Active decrees whose upkeep was paid: they are in effect this turn.
    pub paid: Vec<DecreeId>,
    /// Gold paid this turn.
    pub gold_paid: u64,
    /// Active decrees the treasury could not pay: revoked automatically,
    /// exactly like a revocation (cooldown), so they have no effect this turn.
    /// Listed so the agent sees that the gold ran out.
    pub revoked_unpaid: Vec<DecreeId>,
}

/// One nation's decrees. Decrees missing from `states` are inactive and may
/// be enacted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecreeBook {
    states: BTreeMap<DecreeId, DecreeState>,
}

impl DecreeBook {
    pub fn state(&self, decree: &DecreeId) -> Option<DecreeState> {
        self.states.get(decree).copied()
    }

    /// Every decree with a state, ascending ID.
    pub fn states(&self) -> &BTreeMap<DecreeId, DecreeState> {
        &self.states
    }

    /// Decrees in effect this turn, ascending ID.
    pub fn active(&self) -> impl Iterator<Item = &DecreeId> + '_ {
        self.states
            .iter()
            .filter(|(_, state)| matches!(state, DecreeState::Active { .. }))
            .map(|(decree, _)| decree)
    }

    /// Pending and active decrees: those that occupy a slot.
    pub fn used_slots(&self) -> usize {
        self.states
            .values()
            .filter(|state| !matches!(state, DecreeState::Cooldown { .. }))
            .count()
    }

    pub(crate) fn enact(&mut self, decree: DecreeId) {
        debug_assert!(self.state(&decree).is_none());
        self.states.insert(decree, DecreeState::Pending);
    }

    /// Ends a decree (revoked, or its upkeep unpaid); its cooldown starts and
    /// its slot is free. Without a cooldown it is free again at once.
    pub(crate) fn revoke(&mut self, decree: &DecreeId, cooldown_turns: u32) {
        if cooldown_turns == 0 {
            self.states.remove(decree);
        } else {
            self.states.insert(
                decree.clone(),
                DecreeState::Cooldown {
                    turns_left: cooldown_turns,
                },
            );
        }
    }

    /// End of a global turn: pending decrees become active, active ones count
    /// one more turn, cooldowns run down and finished ones are removed.
    pub(crate) fn advance_turn(&mut self) {
        self.states.retain(|_, state| {
            match state {
                DecreeState::Pending => *state = DecreeState::Active { turns_active: 0 },
                DecreeState::Active { turns_active } => *turns_active += 1,
                DecreeState::Cooldown { turns_left } => {
                    *turns_left = turns_left.saturating_sub(1);
                    return *turns_left > 0;
                }
            }
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::economy::ResourceCatalog;

    fn id(id: &str) -> DecreeId {
        DecreeId::new(id).unwrap()
    }

    #[test]
    fn decree_ids_are_snake_case() {
        for valid in ["games", "agricultural_decree", "tier2_bonus"] {
            assert_eq!(DecreeId::new(valid).unwrap().as_str(), valid);
        }
        for invalid in ["", "Games", "free-trade", "_games", "games_", "a__b", "2nd"] {
            assert!(DecreeId::new(invalid).is_none(), "{invalid:?}");
        }
        assert_eq!(serde_json::to_value(id("games")).unwrap(), "games");
        assert!(serde_json::from_str::<DecreeId>(r#""Free Beer""#).is_err());
    }

    #[test]
    fn production_changes_match_category_and_tier_of_a_material() {
        let catalog = ResourceCatalog::standard();
        let material = |name: &str| catalog.get(catalog.resolve(name).unwrap()).unwrap();
        let effects = DecreeEffects {
            production: vec![
                ProductionChange {
                    categories: vec![ResourceCategory::RawFood],
                    tiers: vec![1],
                    change: 0.2,
                },
                ProductionChange {
                    categories: vec![],
                    tiers: vec![],
                    change: -0.15,
                },
            ],
            ..DecreeEffects::default()
        };
        assert!((effects.production_change(material("grain")) - 0.05).abs() < 1e-6);
        assert_eq!(effects.production_change(material("wood")), -0.15);
        assert_eq!(
            DecreeEffects::default().production_change(material("wood")),
            0.0
        );
    }

    #[test]
    fn a_decree_is_pending_then_active_and_cools_down_after_revocation() {
        let games = id("games");
        let mut book = DecreeBook::default();
        book.enact(games.clone());
        assert_eq!(book.used_slots(), 1);
        assert_eq!(book.active().count(), 0, "pending decrees have no effect");
        book.advance_turn();
        assert_eq!(
            book.state(&games),
            Some(DecreeState::Active { turns_active: 0 })
        );
        book.advance_turn();
        assert_eq!(
            book.state(&games),
            Some(DecreeState::Active { turns_active: 1 })
        );
        book.revoke(&games, 3);
        assert_eq!(book.used_slots(), 0, "a cooldown frees the slot");
        for left in (1..3).rev() {
            book.advance_turn();
            assert_eq!(
                book.state(&games),
                Some(DecreeState::Cooldown { turns_left: left })
            );
        }
        book.advance_turn();
        assert_eq!(book.state(&games), None);
        book.enact(games.clone());
        book.revoke(&games, 0);
        assert_eq!(book.state(&games), None, "no cooldown: free at once");
    }

    #[test]
    fn decree_states_serialize_with_a_state_tag() {
        assert_eq!(
            serde_json::to_value(DecreeState::Cooldown { turns_left: 3 }).unwrap(),
            serde_json::json!({"state": "cooldown", "turns_left": 3})
        );
    }
}
