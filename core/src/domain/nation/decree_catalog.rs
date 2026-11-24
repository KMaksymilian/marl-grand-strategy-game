//! Decree content loaded from `decrees.toml` (gameplay specification §7): the
//! common rules (upkeep, duration, cooldown, slots) and every decree with its
//! cost multiplier and effects. Loaded and validated like the resource
//! catalogue: `standard()` is the bundled file, `load` reads one from disk.
use super::decree::{DecreeEffects, DecreeId};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

/// Rules shared by every decree.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecreeRules {
    /// Gold per owned settlement per active turn, before the decree's multiplier.
    pub base_upkeep_per_settlement: u32,
    /// Active turns before a decree can be revoked.
    pub minimum_active_turns: u32,
    /// Turns after revocation before the same decree can be enacted again.
    pub cooldown_turns: u32,
    /// Slots without happiness bonuses.
    pub base_slots: usize,
    pub max_slots: usize,
    /// Average happiness that gives the first extra slot.
    pub extra_slot_happiness: f32,
    /// Further happiness points for each next extra slot.
    pub extra_slot_happiness_step: f32,
}

impl DecreeRules {
    /// Gold per active turn: `base × settlements × cost_multiplier`.
    pub fn upkeep(&self, cost_multiplier: u32, settlements: usize) -> u64 {
        u64::from(self.base_upkeep_per_settlement)
            .saturating_mul(u64::try_from(settlements).unwrap_or(u64::MAX))
            .saturating_mul(u64::from(cost_multiplier))
    }

    /// Slots for a nation whose settlements have this average happiness: the
    /// base, one more at `extra_slot_happiness` and one more for every further
    /// step, at most `max_slots`. Without happiness (None) the base slots.
    pub fn slots(&self, average_happiness: Option<f32>) -> usize {
        match average_happiness {
            Some(happiness) if happiness >= self.extra_slot_happiness => {
                let steps =
                    (happiness - self.extra_slot_happiness) / self.extra_slot_happiness_step;
                let extra = (steps.floor() as usize).saturating_add(1);
                self.base_slots.saturating_add(extra).min(self.max_slots)
            }
            _ => self.base_slots,
        }
    }
}

/// One decree of the catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct DecreeDefinition {
    id: DecreeId,
    name: String,
    cost_multiplier: u32,
    effects: DecreeEffects,
}

impl DecreeDefinition {
    pub fn id(&self) -> &DecreeId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Multiplier of the base upkeep (§7 table).
    pub fn cost_multiplier(&self) -> u32 {
        self.cost_multiplier
    }

    /// The decree's advantage and drawback (§7 table).
    pub fn effects(&self) -> &DecreeEffects {
        &self.effects
    }
}

#[derive(Debug)]
pub enum DecreeCatalogError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    InvalidIdentifier(String),
    DuplicateDecree(DecreeId),
    EmptyName(DecreeId),
    ZeroCostMultiplier(DecreeId),
    InvalidProductionTier {
        decree: DecreeId,
        tier: u8,
    },
    /// An effect value is NaN or infinite.
    NonFiniteEffect(DecreeId),
    /// The rules contradict themselves; the text names the rule.
    InvalidRules(&'static str),
}

impl fmt::Display for DecreeCatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read decree catalogue: {error}"),
            Self::Parse(error) => write!(f, "invalid decree catalogue TOML: {error}"),
            Self::InvalidIdentifier(id) => write!(f, "invalid decree identifier {id:?}"),
            Self::DuplicateDecree(id) => write!(f, "duplicate decree {id}"),
            Self::EmptyName(id) => write!(f, "decree {id} has an empty name"),
            Self::ZeroCostMultiplier(id) => write!(f, "decree {id} has a zero cost multiplier"),
            Self::InvalidProductionTier { decree, tier } => write!(
                f,
                "decree {decree} changes production of tier {tier}; expected 1 through 3"
            ),
            Self::NonFiniteEffect(id) => write!(f, "decree {id} has a non-finite effect"),
            Self::InvalidRules(rule) => write!(f, "invalid decree rules: {rule}"),
        }
    }
}

impl std::error::Error for DecreeCatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Parse(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    rules: DecreeRules,
    #[serde(default)]
    decrees: Vec<RawDecree>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDecree {
    id: String,
    name: String,
    cost_multiplier: u32,
    #[serde(default)]
    effects: DecreeEffects,
}

/// Every decree a nation may enact, in file order, and the common rules.
#[derive(Debug, Clone, PartialEq)]
pub struct DecreeCatalog {
    rules: DecreeRules,
    definitions: Vec<DecreeDefinition>,
    index: HashMap<DecreeId, usize>,
}

impl DecreeCatalog {
    /// The bundled `decrees.toml`.
    pub fn standard() -> &'static Self {
        static CATALOG: OnceLock<DecreeCatalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            Self::from_toml(include_str!("../../../../decrees.toml"))
                .expect("the bundled decree catalogue must be valid")
        })
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, DecreeCatalogError> {
        let source = std::fs::read_to_string(path).map_err(DecreeCatalogError::Read)?;
        Self::from_toml(&source)
    }

    pub fn from_toml(source: &str) -> Result<Self, DecreeCatalogError> {
        let raw: RawCatalog = toml::from_str(source).map_err(DecreeCatalogError::Parse)?;
        let rules = raw.rules;
        if rules.base_slots > rules.max_slots {
            return Err(DecreeCatalogError::InvalidRules(
                "base_slots must not exceed max_slots",
            ));
        }
        if !rules.extra_slot_happiness.is_finite() {
            return Err(DecreeCatalogError::InvalidRules(
                "extra_slot_happiness must be finite",
            ));
        }
        if !(rules.extra_slot_happiness_step.is_finite() && rules.extra_slot_happiness_step > 0.0) {
            return Err(DecreeCatalogError::InvalidRules(
                "extra_slot_happiness_step must be positive",
            ));
        }
        let mut catalog = Self {
            rules,
            definitions: Vec::new(),
            index: HashMap::new(),
        };
        for decree in raw.decrees {
            catalog.insert(decree)?;
        }
        Ok(catalog)
    }

    pub fn rules(&self) -> &DecreeRules {
        &self.rules
    }

    pub fn get(&self, decree: &DecreeId) -> Option<&DecreeDefinition> {
        self.index
            .get(decree)
            .map(|&index| &self.definitions[index])
    }

    /// Every decree, in file order.
    pub fn definitions(&self) -> &[DecreeDefinition] {
        &self.definitions
    }

    /// Gold per active turn of one decree for a nation with `settlements`
    /// settlements; None for a decree that is not in the catalogue.
    pub fn upkeep(&self, decree: &DecreeId, settlements: usize) -> Option<u64> {
        self.get(decree)
            .map(|definition| self.rules.upkeep(definition.cost_multiplier, settlements))
    }

    /// Combined effects of several decrees; IDs not in the catalogue add nothing.
    pub fn effects_of<'a>(&self, decrees: impl IntoIterator<Item = &'a DecreeId>) -> DecreeEffects {
        let mut combined = DecreeEffects::default();
        for definition in decrees.into_iter().filter_map(|decree| self.get(decree)) {
            combined.combine(&definition.effects);
        }
        combined
    }

    fn insert(&mut self, raw: RawDecree) -> Result<(), DecreeCatalogError> {
        let id = DecreeId::new(&raw.id).ok_or(DecreeCatalogError::InvalidIdentifier(raw.id))?;
        if self.index.contains_key(&id) {
            return Err(DecreeCatalogError::DuplicateDecree(id));
        }
        if raw.name.trim().is_empty() {
            return Err(DecreeCatalogError::EmptyName(id));
        }
        if raw.cost_multiplier == 0 {
            return Err(DecreeCatalogError::ZeroCostMultiplier(id));
        }
        if let Some(&tier) = raw
            .effects
            .production
            .iter()
            .flat_map(|change| &change.tiers)
            .find(|tier| !(1..=3).contains(*tier))
        {
            return Err(DecreeCatalogError::InvalidProductionTier { decree: id, tier });
        }
        if !raw.effects.numbers().all(f32::is_finite) {
            return Err(DecreeCatalogError::NonFiniteEffect(id));
        }
        self.index.insert(id.clone(), self.definitions.len());
        self.definitions.push(DecreeDefinition {
            id,
            name: raw.name,
            cost_multiplier: raw.cost_multiplier,
            effects: raw.effects,
        });
        Ok(())
    }
}
