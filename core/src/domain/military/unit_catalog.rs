//! Unit types and military rules loaded from `units.toml` (gameplay
//! specification §10 and §11). Loaded and validated like the resource
//! catalogue: `standard()` is the bundled file, `load` reads one from disk.
//! Equipment names resolve against a resource catalogue.
use crate::domain::economy::{ResourceCatalog, ResourceType};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// Identifier of a unit type, as written in `units.toml` and in agent JSON:
/// snake case (`"siege_engines"`). Whether the type exists is checked against
/// the catalogue when an action is validated.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitTypeId(Arc<str>);

impl UnitTypeId {
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

impl fmt::Debug for UnitTypeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl fmt::Display for UnitTypeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for UnitTypeId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for UnitTypeId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let id = String::deserialize(deserializer)?;
        Self::new(&id)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid unit type ID {id:?}")))
    }
}

/// Rules shared by every unit type.
#[derive(Debug, Clone, PartialEq)]
pub struct MilitaryRules {
    /// People in one unit (§10.1).
    pub soldiers_per_unit: u32,
    /// Share of a settlement's population that may serve at the same time
    /// (§10.2), before decree multipliers.
    pub conscription_share: f32,
    /// Turns from the order to a ready unit (§10.2).
    pub recruitment_turns: u32,
    /// Attack or defence multiplier against a type the unit has an advantage
    /// over (§10.1).
    pub advantage_multiplier: f32,
    /// Morale (§10.3).
    pub initial_morale: u32,
    pub max_morale: u32,
    pub morale_gain_full_upkeep: u32,
    /// Instead of `morale_gain_full_upkeep` when all food is tier 2.
    pub morale_gain_tier2_food: u32,
    pub morale_loss_partial_upkeep: u32,
    /// Battles (§11.3).
    pub loser_unit_losses: f32,
    pub loser_morale_loss: u32,
    pub strength_variance: f32,
    /// Garrison (§11.4).
    pub garrison_unit: UnitTypeId,
    pub garrison_share: f32,
}

/// One unit type of the catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitDefinition {
    id: UnitTypeId,
    name: String,
    equipment: BTreeMap<ResourceType, u32>,
    attack: u32,
    defense: u32,
    movement: u32,
    advantage_against: Vec<UnitTypeId>,
    wall_bonus_reduction: f32,
    gold_upkeep: u32,
    food_upkeep: u32,
}

impl UnitDefinition {
    pub fn id(&self) -> &UnitTypeId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Materials taken from the national stock to recruit one unit.
    pub fn equipment(&self) -> &BTreeMap<ResourceType, u32> {
        &self.equipment
    }

    pub fn attack(&self) -> u32 {
        self.attack
    }

    pub fn defense(&self) -> u32 {
        self.defense
    }

    /// Provinces per turn (§11.1).
    pub fn movement(&self) -> u32 {
        self.movement
    }

    /// Unit types this one beats (§10.1).
    pub fn advantage_against(&self) -> &[UnitTypeId] {
        &self.advantage_against
    }

    /// Wall bonus each unit of this type removes when attacking (§11.4).
    pub fn wall_bonus_reduction(&self) -> f32 {
        self.wall_bonus_reduction
    }

    /// Gold per soldier per turn (§10.3).
    pub fn gold_upkeep(&self) -> u32 {
        self.gold_upkeep
    }

    /// Food per soldier per turn (§10.3).
    pub fn food_upkeep(&self) -> u32 {
        self.food_upkeep
    }
}

#[derive(Debug)]
pub enum UnitCatalogError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    InvalidIdentifier(String),
    DuplicateUnit(UnitTypeId),
    EmptyName(UnitTypeId),
    /// A unit that does not move.
    ZeroMovement(UnitTypeId),
    /// An equipment resource missing from the resource catalogue.
    UnknownEquipment {
        unit: UnitTypeId,
        resource: String,
    },
    ZeroEquipment {
        unit: UnitTypeId,
        resource: String,
    },
    /// An advantage against a type missing from the catalogue, or itself.
    InvalidAdvantage {
        unit: UnitTypeId,
        against: String,
    },
    InvalidWallBonusReduction(UnitTypeId),
    /// The garrison unit is not in the catalogue.
    UnknownGarrisonUnit(String),
    /// The rules contradict themselves; the text names the rule.
    InvalidRules(&'static str),
}

impl fmt::Display for UnitCatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read unit catalogue: {error}"),
            Self::Parse(error) => write!(f, "invalid unit catalogue TOML: {error}"),
            Self::InvalidIdentifier(id) => write!(f, "invalid unit type identifier {id:?}"),
            Self::DuplicateUnit(id) => write!(f, "duplicate unit type {id}"),
            Self::EmptyName(id) => write!(f, "unit type {id} has an empty name"),
            Self::ZeroMovement(id) => write!(f, "unit type {id} has zero movement"),
            Self::UnknownEquipment { unit, resource } => {
                write!(f, "unit type {unit} needs unknown material {resource:?}")
            }
            Self::ZeroEquipment { unit, resource } => {
                write!(f, "unit type {unit} needs zero {resource:?}")
            }
            Self::InvalidAdvantage { unit, against } => {
                write!(
                    f,
                    "unit type {unit} has an advantage against invalid type {against:?}"
                )
            }
            Self::InvalidWallBonusReduction(id) => write!(
                f,
                "unit type {id} has a wall bonus reduction outside 0.0 through 1.0"
            ),
            Self::UnknownGarrisonUnit(id) => write!(f, "unknown garrison unit type {id:?}"),
            Self::InvalidRules(rule) => write!(f, "invalid military rules: {rule}"),
        }
    }
}

impl std::error::Error for UnitCatalogError {
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
    rules: RawRules,
    #[serde(default)]
    units: Vec<RawUnit>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRules {
    soldiers_per_unit: u32,
    conscription_share: f32,
    recruitment_turns: u32,
    advantage_multiplier: f32,
    initial_morale: u32,
    max_morale: u32,
    morale_gain_full_upkeep: u32,
    morale_gain_tier2_food: u32,
    morale_loss_partial_upkeep: u32,
    loser_unit_losses: f32,
    loser_morale_loss: u32,
    strength_variance: f32,
    garrison_unit: String,
    garrison_share: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnit {
    id: String,
    name: String,
    #[serde(default)]
    equipment: BTreeMap<String, u32>,
    attack: u32,
    defense: u32,
    movement: u32,
    #[serde(default)]
    advantage_against: Vec<String>,
    #[serde(default)]
    wall_bonus_reduction: f32,
    gold_upkeep: u32,
    food_upkeep: u32,
}

/// Every unit type a nation may recruit, in file order, and the military rules.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitCatalog {
    rules: MilitaryRules,
    definitions: Vec<UnitDefinition>,
    index: HashMap<UnitTypeId, usize>,
}

fn is_share(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

impl UnitCatalog {
    /// The bundled `units.toml`, with equipment from the bundled resources.
    pub fn standard() -> &'static Self {
        static CATALOG: OnceLock<UnitCatalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            Self::from_toml(
                include_str!("../../../../units.toml"),
                ResourceCatalog::standard(),
            )
            .expect("the bundled unit catalogue must be valid")
        })
    }

    pub fn load(
        path: impl AsRef<Path>,
        resources: &ResourceCatalog,
    ) -> Result<Self, UnitCatalogError> {
        let source = std::fs::read_to_string(path).map_err(UnitCatalogError::Read)?;
        Self::from_toml(&source, resources)
    }

    pub fn from_toml(source: &str, resources: &ResourceCatalog) -> Result<Self, UnitCatalogError> {
        let raw: RawCatalog = toml::from_str(source).map_err(UnitCatalogError::Parse)?;
        let mut definitions = Vec::new();
        let mut index = HashMap::new();
        for unit in &raw.units {
            let id = UnitTypeId::new(&unit.id)
                .ok_or_else(|| UnitCatalogError::InvalidIdentifier(unit.id.clone()))?;
            if index.insert(id.clone(), definitions.len()).is_some() {
                return Err(UnitCatalogError::DuplicateUnit(id));
            }
            definitions.push(Self::definition(id, unit, resources)?);
        }
        // Advantages may name types defined later in the file.
        for (definition, raw_unit) in definitions.iter_mut().zip(&raw.units) {
            for against in &raw_unit.advantage_against {
                let target = UnitTypeId::new(against)
                    .filter(|target| index.contains_key(target) && *target != definition.id)
                    .ok_or_else(|| UnitCatalogError::InvalidAdvantage {
                        unit: definition.id.clone(),
                        against: against.clone(),
                    })?;
                definition.advantage_against.push(target);
            }
        }
        let rules = Self::rules_from(raw.rules, &index)?;
        Ok(Self {
            rules,
            definitions,
            index,
        })
    }

    fn definition(
        id: UnitTypeId,
        raw: &RawUnit,
        resources: &ResourceCatalog,
    ) -> Result<UnitDefinition, UnitCatalogError> {
        if raw.name.trim().is_empty() {
            return Err(UnitCatalogError::EmptyName(id));
        }
        if raw.movement == 0 {
            return Err(UnitCatalogError::ZeroMovement(id));
        }
        if !is_share(raw.wall_bonus_reduction) {
            return Err(UnitCatalogError::InvalidWallBonusReduction(id));
        }
        let mut equipment = BTreeMap::new();
        for (name, &amount) in &raw.equipment {
            let resource =
                resources
                    .resolve(name)
                    .ok_or_else(|| UnitCatalogError::UnknownEquipment {
                        unit: id.clone(),
                        resource: name.clone(),
                    })?;
            if amount == 0 {
                return Err(UnitCatalogError::ZeroEquipment {
                    unit: id,
                    resource: name.clone(),
                });
            }
            *equipment.entry(resource).or_insert(0) += amount;
        }
        Ok(UnitDefinition {
            id,
            name: raw.name.clone(),
            equipment,
            attack: raw.attack,
            defense: raw.defense,
            movement: raw.movement,
            advantage_against: Vec::new(),
            wall_bonus_reduction: raw.wall_bonus_reduction,
            gold_upkeep: raw.gold_upkeep,
            food_upkeep: raw.food_upkeep,
        })
    }

    fn rules_from(
        raw: RawRules,
        index: &HashMap<UnitTypeId, usize>,
    ) -> Result<MilitaryRules, UnitCatalogError> {
        let garrison_unit = UnitTypeId::new(&raw.garrison_unit)
            .filter(|id| index.contains_key(id))
            .ok_or(UnitCatalogError::UnknownGarrisonUnit(raw.garrison_unit))?;
        if raw.soldiers_per_unit == 0 {
            return Err(UnitCatalogError::InvalidRules(
                "soldiers_per_unit must be positive",
            ));
        }
        if raw.max_morale == 0 || raw.initial_morale > raw.max_morale {
            return Err(UnitCatalogError::InvalidRules(
                "initial_morale must be at most a positive max_morale",
            ));
        }
        if !(raw.advantage_multiplier.is_finite() && raw.advantage_multiplier > 0.0) {
            return Err(UnitCatalogError::InvalidRules(
                "advantage_multiplier must be positive",
            ));
        }
        for (share, rule) in [
            (
                raw.conscription_share,
                "conscription_share must be within 0.0 through 1.0",
            ),
            (
                raw.garrison_share,
                "garrison_share must be within 0.0 through 1.0",
            ),
            (
                raw.loser_unit_losses,
                "loser_unit_losses must be within 0.0 through 1.0",
            ),
            (
                raw.strength_variance,
                "strength_variance must be within 0.0 through 1.0",
            ),
        ] {
            if !is_share(share) {
                return Err(UnitCatalogError::InvalidRules(rule));
            }
        }
        Ok(MilitaryRules {
            soldiers_per_unit: raw.soldiers_per_unit,
            conscription_share: raw.conscription_share,
            recruitment_turns: raw.recruitment_turns,
            advantage_multiplier: raw.advantage_multiplier,
            initial_morale: raw.initial_morale,
            max_morale: raw.max_morale,
            morale_gain_full_upkeep: raw.morale_gain_full_upkeep,
            morale_gain_tier2_food: raw.morale_gain_tier2_food,
            morale_loss_partial_upkeep: raw.morale_loss_partial_upkeep,
            loser_unit_losses: raw.loser_unit_losses,
            loser_morale_loss: raw.loser_morale_loss,
            strength_variance: raw.strength_variance,
            garrison_unit,
            garrison_share: raw.garrison_share,
        })
    }

    pub fn rules(&self) -> &MilitaryRules {
        &self.rules
    }

    pub fn get(&self, unit: &UnitTypeId) -> Option<&UnitDefinition> {
        self.index.get(unit).map(|&index| &self.definitions[index])
    }

    /// Every unit type, in file order.
    pub fn definitions(&self) -> &[UnitDefinition] {
        &self.definitions
    }

    /// The unit type of settlement garrisons (§11.4).
    pub fn garrison_unit(&self) -> &UnitDefinition {
        self.get(&self.rules.garrison_unit)
            .expect("validated when loading")
    }

    /// `unit` counts `advantage_multiplier` times against `other` (§10.1).
    pub fn has_advantage(&self, unit: &UnitTypeId, other: &UnitTypeId) -> bool {
        self.get(unit)
            .is_some_and(|definition| definition.advantage_against.contains(other))
    }
}
