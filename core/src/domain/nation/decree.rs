//! Decrees (gameplay specification §7): nation-wide policies with a one-time
//! cost, a delayed start, a minimum duration and a cooldown, plus the one-shot
//! capital relocation.
//!
//! Lifecycle of one decree enacted in turn T:
//!
//! | When | State | Effects |
//! | --- | --- | --- |
//! | turn T (cost paid) | `Pending` | no |
//! | from turn T+1 | `Active` | yes |
//! | revoked after ≥ 5 active turns | `Cooldown` (5 turns) | no |
//!
//! Pending and active decrees use slots; a decree on cooldown does not.
//!
//! The nation layer owns this lifecycle. The effects are only data
//! (`DecreeEffects`): every system that a decree changes reads the combined
//! effects of the owner's active decrees (`World::decree_effects`) and applies
//! them itself. Each field names the system that applies it.
use crate::domain::ids::SettlementId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The decrees of §7. Serialized in snake_case (`"agricultural_decree"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decree {
    /// Dekret rolny.
    AgriculturalDecree,
    /// Reforma rzemiosła.
    CraftReform,
    /// Górnictwo państwowe.
    StateMining,
    /// Wielka budowa.
    GreatConstruction,
    /// Wolny handel.
    FreeTrade,
    /// Protekcjonizm.
    Protectionism,
    /// Kontyngent wojskowy.
    MilitaryLevy,
    /// Mobilizacja.
    Mobilization,
    /// Dyscyplina.
    Discipline,
    /// Racjonowanie jedzenia.
    FoodRationing,
    /// Igrzyska.
    Games,
    /// Mecenat sztuki.
    ArtPatronage,
    /// Fortyfikacja kraju.
    NationalFortification,
}

/// A group of produced resources that a production change applies to.
// ECONOMY (production, §2 and §7): the production system decides which
// resources each scope covers, from the resource catalogue (tier and group).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionScope {
    /// Raw food (§2.1.1).
    FoodTier1,
    /// Raw building materials (§2.1.2).
    BuildingMaterialTier1,
    /// Every tier-1 material.
    Tier1,
    /// Every tier-2 and tier-3 material.
    Tier2And3,
    /// Mined ores and coal (§2.1.3).
    Ore,
    /// Everything produced.
    All,
}

/// Combined effects of a nation's active decrees. Fractions add up
/// (+0.2 and −0.1 give +0.1), multipliers multiply, flags combine with "or".
/// No decree active: every fraction is 0, every multiplier 1, every flag false.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecreeEffects {
    // ECONOMY (production): change of output per scope, as a fraction of it
    // (+0.2 = +20%). Applied by the production system.
    pub production: BTreeMap<ProductionScope, f32>,
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
    /// bronze and cattle; applies once those resources exist.
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
            production: BTreeMap::new(),
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
    /// Combined effects of several decrees.
    pub fn of(decrees: impl IntoIterator<Item = Decree>) -> Self {
        let mut combined = Self::default();
        for decree in decrees {
            combined.combine(&decree.effects());
        }
        combined
    }

    fn combine(&mut self, other: &Self) {
        for (&scope, &change) in &other.production {
            *self.production.entry(scope).or_default() += change;
        }
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
}

impl Decree {
    /// Every decree in a fixed order, e.g. for fixed-size vectors.
    pub const ALL: [Self; 13] = [
        Self::AgriculturalDecree,
        Self::CraftReform,
        Self::StateMining,
        Self::GreatConstruction,
        Self::FreeTrade,
        Self::Protectionism,
        Self::MilitaryLevy,
        Self::Mobilization,
        Self::Discipline,
        Self::FoodRationing,
        Self::Games,
        Self::ArtPatronage,
        Self::NationalFortification,
    ];

    /// Gold per owned settlement of the base cost.
    pub const BASE_COST_PER_SETTLEMENT: u32 = 10;
    /// Turns a decree must be active before it can be revoked.
    pub const MINIMUM_ACTIVE_TURNS: u32 = 5;
    /// Turns after revocation before the same decree can be enacted again.
    pub const COOLDOWN_TURNS: u32 = 5;

    /// Multiplier of the base cost (§7 table).
    pub fn cost_multiplier(self) -> u32 {
        match self {
            Self::GreatConstruction => 2,
            Self::ArtPatronage => 3,
            Self::Games | Self::NationalFortification => 4,
            _ => 1,
        }
    }

    /// The one-time cost of enacting the decree for a nation with
    /// `settlements` settlements, paid when it is enacted.
    pub fn cost(self, settlements: usize) -> u64 {
        u64::from(Self::BASE_COST_PER_SETTLEMENT)
            * u64::try_from(settlements).unwrap_or(u64::MAX)
            * u64::from(self.cost_multiplier())
    }

    /// The decree's advantage and drawback (§7 table).
    pub fn effects(self) -> DecreeEffects {
        use ProductionScope::*;
        let mut effects = DecreeEffects::default();
        match self {
            Self::AgriculturalDecree => {
                effects.production =
                    BTreeMap::from([(FoodTier1, 0.2), (BuildingMaterialTier1, -0.1)]);
            }
            Self::CraftReform => {
                effects.production = BTreeMap::from([(Tier2And3, 0.2), (Tier1, -0.1)]);
            }
            Self::StateMining => {
                effects.production = BTreeMap::from([(Ore, 0.3)]);
                effects.happiness = -5.0;
            }
            Self::GreatConstruction => effects.construction_cost = -0.2,
            Self::FreeTrade => {
                effects.tariff_multiplier = 0.5;
                effects.caravan_speed = 0.25;
            }
            Self::Protectionism => effects.tariff_multiplier = 2.0,
            Self::MilitaryLevy => {
                effects.military_levy_multiplier = 2.0;
                effects.happiness = -5.0;
            }
            Self::Mobilization => {
                effects.conscription_pool_multiplier = 2.0;
                effects.instant_recruitment = true;
                effects.production = BTreeMap::from([(All, -0.15)]);
                effects.happiness = -10.0;
            }
            Self::Discipline => {
                effects.morale_loss_multiplier = 0.5;
                effects.army_upkeep_multiplier = 1.25;
            }
            Self::FoodRationing => {
                effects.food_consumption = -0.25;
                effects.happiness = -15.0;
            }
            Self::Games => effects.happiness = 15.0,
            Self::ArtPatronage => effects.monument_happiness_multiplier = 2.0,
            Self::NationalFortification => {
                effects.wall_bonus_points = 10.0;
                effects.wall_cost = -0.3;
            }
        }
        effects
    }
}

/// Where one decree is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum DecreeState {
    /// Enacted this turn; active from the next turn.
    Pending,
    /// In effect; `turns_active` counts completed active turns.
    Active { turns_active: u32 },
    /// Revoked; can be enacted again when `turns_left` reaches zero.
    Cooldown { turns_left: u32 },
}

/// A paid capital relocation waiting for the start of the next turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapitalRelocation {
    pub target: SettlementId,
    /// Gold paid; refunded when the relocation fails.
    pub paid: u32,
}

impl CapitalRelocation {
    /// Gold per owned settlement for one relocation.
    pub const COST_PER_SETTLEMENT: u32 = 100;

    pub fn cost(settlements: usize) -> u64 {
        u64::from(Self::COST_PER_SETTLEMENT) * u64::try_from(settlements).unwrap_or(u64::MAX)
    }
}

/// Why a capital relocation failed and was refunded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelocationFailure {
    /// The target no longer belongs to the nation (it was conquered).
    NotOwned,
    /// An enemy army stands in the target's province.
    UnderAttack,
}

/// What happened to a capital relocation one turn after it was ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelocationOutcome {
    Completed {
        from: SettlementId,
        to: SettlementId,
    },
    /// The relocation expired and its cost went back to the treasury.
    Refunded {
        target: SettlementId,
        reason: RelocationFailure,
        refund: u32,
    },
}

/// A nation's decrees and pending relocation. Decrees missing from `states`
/// are inactive and may be enacted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecreeBook {
    states: BTreeMap<Decree, DecreeState>,
    relocation: Option<CapitalRelocation>,
}

impl DecreeBook {
    /// Slots at the start of the game.
    pub const BASE_SLOTS: usize = 2;
    pub const MAX_SLOTS: usize = 6;

    /// Slots for a nation whose settlements have this average happiness (§7):
    /// 2, one more at 70 and one more for every further 10 points (6 at 100).
    /// Without happiness (None) a nation has the base slots.
    pub fn slots(average_happiness: Option<f32>) -> usize {
        match average_happiness {
            Some(happiness) if happiness >= 70.0 => {
                let extra = ((happiness - 70.0) / 10.0).floor() as usize + 1;
                (Self::BASE_SLOTS + extra).min(Self::MAX_SLOTS)
            }
            _ => Self::BASE_SLOTS,
        }
    }

    pub fn state(&self, decree: Decree) -> Option<DecreeState> {
        self.states.get(&decree).copied()
    }

    /// Every decree with a state, in `Decree` order.
    pub fn states(&self) -> &BTreeMap<Decree, DecreeState> {
        &self.states
    }

    /// Decrees in effect this turn.
    pub fn active(&self) -> impl Iterator<Item = Decree> + '_ {
        self.states
            .iter()
            .filter(|(_, state)| matches!(state, DecreeState::Active { .. }))
            .map(|(&decree, _)| decree)
    }

    /// Pending and active decrees: those that occupy a slot.
    pub fn used_slots(&self) -> usize {
        self.states
            .values()
            .filter(|state| !matches!(state, DecreeState::Cooldown { .. }))
            .count()
    }

    /// Combined effects of the active decrees.
    pub fn effects(&self) -> DecreeEffects {
        DecreeEffects::of(self.active())
    }

    pub fn relocation(&self) -> Option<CapitalRelocation> {
        self.relocation
    }

    pub(crate) fn enact(&mut self, decree: Decree) {
        debug_assert!(self.state(decree).is_none());
        self.states.insert(decree, DecreeState::Pending);
    }

    /// Ends a decree; its cooldown starts and its slot is free.
    pub(crate) fn revoke(&mut self, decree: Decree) {
        self.states.insert(
            decree,
            DecreeState::Cooldown {
                turns_left: Decree::COOLDOWN_TURNS,
            },
        );
    }

    pub(crate) fn set_relocation(&mut self, relocation: CapitalRelocation) {
        self.relocation = Some(relocation);
    }

    pub(crate) fn take_relocation(&mut self) -> Option<CapitalRelocation> {
        self.relocation.take()
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

    #[test]
    fn costs_follow_the_table_and_scale_with_settlements() {
        assert_eq!(Decree::AgriculturalDecree.cost(3), 30);
        assert_eq!(Decree::GreatConstruction.cost(3), 60);
        assert_eq!(Decree::ArtPatronage.cost(3), 90);
        assert_eq!(Decree::Games.cost(3), 120);
        assert_eq!(Decree::NationalFortification.cost(1), 40);
        assert_eq!(CapitalRelocation::cost(4), 400);
    }

    #[test]
    fn slots_grow_with_average_happiness_from_seventy() {
        assert_eq!(DecreeBook::slots(None), 2);
        assert_eq!(DecreeBook::slots(Some(69.9)), 2);
        assert_eq!(DecreeBook::slots(Some(70.0)), 3);
        assert_eq!(DecreeBook::slots(Some(79.9)), 3);
        assert_eq!(DecreeBook::slots(Some(80.0)), 4);
        assert_eq!(DecreeBook::slots(Some(90.0)), 5);
        assert_eq!(DecreeBook::slots(Some(100.0)), 6);
    }

    #[test]
    fn effects_of_several_decrees_stack() {
        let effects = DecreeEffects::of([
            Decree::AgriculturalDecree,
            Decree::CraftReform,
            Decree::Mobilization,
            Decree::FreeTrade,
            Decree::Protectionism,
            Decree::Games,
            Decree::FoodRationing,
        ]);
        assert_eq!(effects.production[&ProductionScope::FoodTier1], 0.2);
        assert_eq!(effects.production[&ProductionScope::Tier1], -0.1);
        assert_eq!(effects.production[&ProductionScope::All], -0.15);
        assert_eq!(effects.tariff_multiplier, 1.0, "×0.5 × ×2");
        assert_eq!(effects.caravan_speed, 0.25);
        assert_eq!(effects.happiness, 15.0 - 10.0 - 15.0);
        assert_eq!(effects.food_consumption, -0.25);
        assert!(effects.instant_recruitment);
        assert_eq!(DecreeEffects::of([]), DecreeEffects::default());
        // The same scope from two decrees adds up.
        let doubled = DecreeEffects::of([Decree::CraftReform, Decree::CraftReform]);
        assert_eq!(doubled.production[&ProductionScope::Tier1], -0.2);
    }

    #[test]
    fn a_decree_is_pending_then_active_and_cools_down_after_revocation() {
        let mut book = DecreeBook::default();
        book.enact(Decree::Games);
        assert_eq!(book.used_slots(), 1);
        assert_eq!(book.active().count(), 0, "pending decrees have no effect");
        book.advance_turn();
        assert_eq!(
            book.state(Decree::Games),
            Some(DecreeState::Active { turns_active: 0 })
        );
        assert_eq!(book.effects().happiness, 15.0);
        book.advance_turn();
        assert_eq!(
            book.state(Decree::Games),
            Some(DecreeState::Active { turns_active: 1 })
        );
        book.revoke(Decree::Games);
        assert_eq!(book.used_slots(), 0, "a cooldown frees the slot");
        for left in (1..Decree::COOLDOWN_TURNS).rev() {
            book.advance_turn();
            assert_eq!(
                book.state(Decree::Games),
                Some(DecreeState::Cooldown { turns_left: left })
            );
        }
        book.advance_turn();
        assert_eq!(book.state(Decree::Games), None);
    }

    #[test]
    fn decree_names_serialize_in_snake_case() {
        assert_eq!(
            serde_json::to_value(Decree::AgriculturalDecree).unwrap(),
            "agricultural_decree"
        );
        assert_eq!(
            serde_json::to_value(DecreeState::Cooldown { turns_left: 3 }).unwrap(),
            serde_json::json!({"state": "cooldown", "turns_left": 3})
        );
        for decree in Decree::ALL {
            let json = serde_json::to_string(&decree).unwrap();
            assert_eq!(serde_json::from_str::<Decree>(&json).unwrap(), decree);
        }
    }
}
