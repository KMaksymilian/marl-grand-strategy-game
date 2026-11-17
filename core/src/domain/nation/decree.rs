//! Decrees (gameplay specification §7): nation-wide policies with a recurring
//! cost, a delayed start, a minimum duration and a cooldown, plus the one-shot
//! capital relocation.
//!
//! Lifecycle of one decree (turn T is when the nation enacts it):
//!
//! | When | State | Effects | Upkeep |
//! | --- | --- | --- | --- |
//! | turn T | `Pending` | no | no |
//! | turns T+1.. | `Active` | yes | every turn |
//! | revoked after ≥ 5 active turns, or upkeep unpaid | `Cooldown` (5 turns) | no | no |
//!
//! Pending and active decrees use slots; a decree on cooldown does not.
//! Effects are data: each owning system reads the combined `DecreeEffects` of
//! the owner's active decrees (production, construction cost and caravan speed
//! today; tariffs, happiness and the military when those systems exist).
use crate::domain::economy::{ResourceCategory, ResourceType};
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

/// Which produced resources a production change applies to.
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
    /// Mined ores and coal.
    Ore,
    /// Everything produced.
    All,
}

impl ProductionScope {
    pub fn contains(self, resource: ResourceType) -> bool {
        let tier = resource.tier();
        let category = resource.category();
        match self {
            Self::FoodTier1 => tier == Some(1) && category == Some(ResourceCategory::Food),
            Self::BuildingMaterialTier1 => {
                tier == Some(1) && category == Some(ResourceCategory::BuildingMaterial)
            }
            Self::Tier1 => tier == Some(1),
            Self::Tier2And3 => matches!(tier, Some(2) | Some(3)),
            Self::Ore => resource.is_ore(),
            Self::All => tier.is_some(),
        }
    }
}

/// Combined effects of a nation's active decrees. Fractions add up
/// (+0.2 and −0.1 give +0.1); multipliers multiply; flags combine with "or".
/// A field without a system yet is kept for that system to read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecreeEffects {
    /// Production changes per scope, as fractions of output (+0.2 = +20%).
    pub production: BTreeMap<ProductionScope, f64>,
    /// Material cost of new constructions and upgrades, as a fraction (−0.2).
    pub construction_cost: f64,
    /// Caravan travel speed, as a fraction (+0.25).
    pub caravan_speed: f64,
    /// Multiplier of the nation's customs duties (diplomacy, §9).
    pub tariff_multiplier: f64,
    /// Happiness points in every settlement (happiness model, §4.2).
    pub happiness: f64,
    /// Multiplier of the monuments' happiness (§4.2).
    pub monument_happiness_multiplier: f64,
    /// Multiplier of the production tax on planks, steel, bronze and cattle (§6);
    /// applies once those resources exist.
    pub military_levy_multiplier: f64,
    /// Food consumption, as a fraction (−0.25) (population needs, §4.1).
    pub food_consumption: f64,
    /// Multiplier of the conscription pool (§10.2).
    pub conscription_pool_multiplier: f64,
    /// Recruitment takes no time (§10.2).
    pub instant_recruitment: bool,
    /// Multiplier of army morale losses (§10.3).
    pub morale_loss_multiplier: f64,
    /// Multiplier of army upkeep (§10.3).
    pub army_upkeep_multiplier: f64,
    /// Percentage points added to the wall bonus (§11.4).
    pub wall_bonus_points: f64,
    /// Material cost of walls, as a fraction (−0.3) (§3.4).
    pub wall_cost: f64,
}

impl Default for DecreeEffects {
    fn default() -> Self {
        Self {
            production: BTreeMap::new(),
            construction_cost: 0.0,
            caravan_speed: 0.0,
            tariff_multiplier: 1.0,
            happiness: 0.0,
            monument_happiness_multiplier: 1.0,
            military_levy_multiplier: 1.0,
            food_consumption: 0.0,
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
    /// The production change of one resource: the sum of every matching scope.
    pub fn production_change(&self, resource: ResourceType) -> f64 {
        self.production
            .iter()
            .filter(|(scope, _)| scope.contains(resource))
            .map(|(_, change)| change)
            .sum()
    }

    /// Adds another decree's effects to these.
    fn combine(&mut self, other: &Self) {
        for (&scope, &change) in &other.production {
            *self.production.entry(scope).or_default() += change;
        }
        self.construction_cost += other.construction_cost;
        self.caravan_speed += other.caravan_speed;
        self.tariff_multiplier *= other.tariff_multiplier;
        self.happiness += other.happiness;
        self.monument_happiness_multiplier *= other.monument_happiness_multiplier;
        self.military_levy_multiplier *= other.military_levy_multiplier;
        self.food_consumption += other.food_consumption;
        self.conscription_pool_multiplier *= other.conscription_pool_multiplier;
        self.instant_recruitment |= other.instant_recruitment;
        self.morale_loss_multiplier *= other.morale_loss_multiplier;
        self.army_upkeep_multiplier *= other.army_upkeep_multiplier;
        self.wall_bonus_points += other.wall_bonus_points;
        self.wall_cost += other.wall_cost;
    }

    /// Combined effects of several decrees.
    pub fn of(decrees: impl IntoIterator<Item = Decree>) -> Self {
        let mut combined = Self::default();
        for decree in decrees {
            combined.combine(&decree.effects());
        }
        combined
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

    /// Gold per owned settlement per turn of the base cost.
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

    /// Gold per turn for a nation with `settlements` settlements.
    pub fn upkeep(self, settlements: usize) -> u64 {
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
    /// Revoked or lapsed; can be enacted again when `turns_left` reaches zero.
    Cooldown { turns_left: u32 },
}

/// A paid capital relocation waiting for the next turn.
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
    /// The target no longer belongs to the nation (e.g. it was conquered).
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

/// Decrees in one turn of one nation, for the nation's report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecreeTurnReport {
    /// Decrees in effect this turn (their upkeep was paid).
    pub active: Vec<Decree>,
    /// Upkeep paid this turn.
    pub upkeep_paid: u64,
    /// Active decrees whose upkeep the treasury could not pay: they ended and
    /// their cooldown started.
    pub lapsed: Vec<Decree>,
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

    /// Slots for a nation whose settlements have this average happiness:
    /// 2, plus one at 70 and one more for every further 10 points (6 at 100).
    /// Happiness is not modelled yet, so callers pass None and get 2.
    pub fn slots(average_happiness: Option<f64>) -> usize {
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
    pub fn states(&self) -> impl Iterator<Item = (Decree, DecreeState)> + '_ {
        self.states.iter().map(|(&decree, &state)| (decree, state))
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

    /// Ends an active decree; the cooldown starts. Also used when upkeep is unpaid.
    pub(crate) fn end(&mut self, decree: Decree) {
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
    /// Returns the decrees that become active.
    pub(crate) fn advance_turn(&mut self) -> Vec<Decree> {
        let mut activated = Vec::new();
        self.states.retain(|&decree, state| {
            match state {
                DecreeState::Pending => {
                    *state = DecreeState::Active { turns_active: 0 };
                    activated.push(decree);
                }
                DecreeState::Active { turns_active } => *turns_active += 1,
                DecreeState::Cooldown { turns_left } => {
                    *turns_left = turns_left.saturating_sub(1);
                    return *turns_left > 0;
                }
            }
            true
        });
        activated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ResourceType::{Grain, Labour, Stone, Wood};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn costs_follow_the_table_and_scale_with_settlements() {
        assert_eq!(Decree::AgriculturalDecree.upkeep(3), 30);
        assert_eq!(Decree::GreatConstruction.upkeep(3), 60);
        assert_eq!(Decree::ArtPatronage.upkeep(3), 90);
        assert_eq!(Decree::Games.upkeep(3), 120);
        assert_eq!(Decree::NationalFortification.upkeep(1), 40);
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
    fn production_scopes_select_resource_groups() {
        assert!(ProductionScope::FoodTier1.contains(Grain));
        assert!(!ProductionScope::FoodTier1.contains(Wood));
        assert!(ProductionScope::BuildingMaterialTier1.contains(Stone));
        assert!(ProductionScope::Tier1.contains(Wood));
        assert!(!ProductionScope::Tier2And3.contains(Wood));
        assert!(!ProductionScope::Ore.contains(Stone));
        assert!(ProductionScope::All.contains(Grain));
        assert!(!ProductionScope::All.contains(Labour));
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
        // Grain: +20% (food) − 10% (tier 1) − 15% (all).
        assert!(close(effects.production_change(Grain), -0.05));
        // Wood: −10% (building material) − 10% (tier 1) − 15% (all).
        assert!(close(effects.production_change(Wood), -0.35));
        assert!(close(effects.tariff_multiplier, 1.0), "×0.5 × ×2");
        assert!(close(effects.caravan_speed, 0.25));
        assert!(close(effects.happiness, 15.0 - 10.0 - 15.0));
        assert!(effects.instant_recruitment);
        assert_eq!(DecreeEffects::of([]), DecreeEffects::default());
    }

    #[test]
    fn a_decree_is_pending_then_active_and_cools_down_after_its_end() {
        let mut book = DecreeBook::default();
        book.enact(Decree::Games);
        assert_eq!(book.used_slots(), 1);
        assert_eq!(book.active().count(), 0, "pending decrees have no effect");
        assert_eq!(book.advance_turn(), vec![Decree::Games]);
        assert_eq!(
            book.state(Decree::Games),
            Some(DecreeState::Active { turns_active: 0 })
        );
        assert!(close(book.effects().happiness, 15.0));
        book.advance_turn();
        assert_eq!(
            book.state(Decree::Games),
            Some(DecreeState::Active { turns_active: 1 })
        );
        book.end(Decree::Games);
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
