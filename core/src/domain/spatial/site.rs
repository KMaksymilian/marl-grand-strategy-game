//! Data-driven requirements shared by construction and production.
use super::{Position, TerritoryError};
use crate::domain::economy::{ResourceCatalog, ResourceType};
use crate::domain::world::terrain::biome::Biome;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

pub const TILE_COASTAL: &str = "tile.coastal";
pub const TILE_BIOME: &str = "tile.biome";
pub const TILE_DEPOSITS: &str = "tile.deposits";
pub const PROVINCE_DEPOSITS: &str = "province.deposits";

const BIOMES: [(Biome, &str); 9] = [
    (Biome::Ocean, "ocean"),
    (Biome::Desert, "desert"),
    (Biome::Grassland, "grassland"),
    (Biome::Forest, "forest"),
    (Biome::Rainforest, "rainforest"),
    (Biome::Hills, "hills"),
    (Biome::Taiga, "taiga"),
    (Biome::Tundra, "tundra"),
    (Biome::Snow, "snow"),
];

/// Scalar facts can be added to scenarios without changing the evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteFactKind {
    Bool,
    Integer,
    Id,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteValue {
    Bool(bool),
    Integer(i64),
    Id(String),
    Deposits(Vec<ResourceType>),
}

impl SiteValue {
    pub fn kind(&self) -> Option<SiteFactKind> {
        match self {
            Self::Bool(_) => Some(SiteFactKind::Bool),
            Self::Integer(_) => Some(SiteFactKind::Integer),
            Self::Id(_) => Some(SiteFactKind::Id),
            Self::Deposits(_) => None,
        }
    }
}

impl From<Biome> for SiteValue {
    fn from(biome: Biome) -> Self {
        let (_, id) = BIOMES
            .iter()
            .find(|(candidate, _)| *candidate == biome)
            .expect("all biomes have a fact identifier");
        Self::Id((*id).to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiteEvaluation {
    pub efficiency_percent: u8,
    pub biome: Option<Biome>,
}

impl Default for SiteEvaluation {
    fn default() -> Self {
        Self {
            efficiency_percent: 100,
            biome: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SiteRequirements {
    conditions: Vec<Condition>,
    biome_efficiency: Option<BTreeMap<String, u8>>,
}

#[derive(Debug, Clone)]
struct Condition {
    fact: String,
    predicate: Predicate,
}

#[derive(Debug, Clone)]
enum Predicate {
    Equals(SiteValue),
    AtLeast(i64),
    Contains(ResourceType),
}

impl Predicate {
    fn matches(&self, actual: Option<SiteValue>) -> bool {
        match (self, actual) {
            (Self::Equals(expected), Some(actual)) => *expected == actual,
            (Self::AtLeast(minimum), Some(SiteValue::Integer(actual))) => actual >= *minimum,
            (Self::Contains(resource), Some(SiteValue::Deposits(actual))) => {
                actual.contains(resource)
            }
            _ => false,
        }
    }
}

#[derive(Debug)]
pub enum SiteConfigError {
    Parse(toml::de::Error),
    UnknownFact(String),
    DuplicateCondition(String),
    UnknownBiome(String),
    EmptyBiomeEfficiency,
    InvalidEfficiency {
        biome: String,
        percent: u8,
    },
    UnknownDeposit(String),
    InvalidCondition {
        fact: String,
        expected: &'static str,
    },
}

impl fmt::Display for SiteConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "invalid site requirements TOML: {error}"),
            Self::UnknownFact(fact) => write!(formatter, "unknown site fact {fact:?}"),
            Self::DuplicateCondition(fact) => {
                write!(formatter, "duplicate site condition for {fact:?}")
            }
            Self::UnknownBiome(biome) => write!(formatter, "unknown site biome {biome:?}"),
            Self::EmptyBiomeEfficiency => write!(formatter, "site efficiency defines no biomes"),
            Self::InvalidEfficiency { biome, percent } => {
                write!(
                    formatter,
                    "invalid site efficiency {percent}% for {biome:?}"
                )
            }
            Self::UnknownDeposit(resource) => {
                write!(formatter, "unknown site deposit {resource:?}")
            }
            Self::InvalidCondition { fact, expected } => {
                write!(formatter, "site condition for {fact:?} expects {expected}")
            }
        }
    }
}

impl std::error::Error for SiteConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequirements {
    #[serde(default)]
    site_requirements: Vec<RawCondition>,
    biome_efficiency: Option<BTreeMap<String, u8>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCondition {
    fact: String,
    op: Operator,
    value: toml::Value,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operator {
    Eq,
    Gte,
    Contains,
}

impl RawCondition {
    fn resolve(
        self,
        resources: &ResourceCatalog,
        configured_fact_types: &HashMap<String, SiteFactKind>,
    ) -> Result<Condition, SiteConfigError> {
        let invalid = |expected| SiteConfigError::InvalidCondition {
            fact: self.fact.clone(),
            expected,
        };
        let predicate = if matches!(self.fact.as_str(), TILE_DEPOSITS | PROVINCE_DEPOSITS) {
            let (Operator::Contains, toml::Value::String(resource)) = (self.op, self.value) else {
                return Err(invalid("contains and a material identifier"));
            };
            Predicate::Contains(
                resources
                    .resolve(&resource)
                    .ok_or(SiteConfigError::UnknownDeposit(resource))?,
            )
        } else {
            let kind = match self.fact.as_str() {
                TILE_COASTAL => SiteFactKind::Bool,
                TILE_BIOME => SiteFactKind::Id,
                name => *configured_fact_types
                    .get(name)
                    .ok_or_else(|| SiteConfigError::UnknownFact(self.fact.clone()))?,
            };
            match (kind, self.op, self.value) {
                (SiteFactKind::Bool, Operator::Eq, toml::Value::Boolean(value)) => {
                    Predicate::Equals(SiteValue::Bool(value))
                }
                (SiteFactKind::Integer, Operator::Eq, toml::Value::Integer(value)) => {
                    Predicate::Equals(SiteValue::Integer(value))
                }
                (SiteFactKind::Integer, Operator::Gte, toml::Value::Integer(value)) => {
                    Predicate::AtLeast(value)
                }
                (SiteFactKind::Id, Operator::Eq, toml::Value::String(value)) => {
                    if self.fact == TILE_BIOME && !BIOMES.iter().any(|(_, id)| *id == value) {
                        return Err(SiteConfigError::UnknownBiome(value));
                    }
                    Predicate::Equals(SiteValue::Id(value))
                }
                _ => {
                    return Err(invalid(match kind {
                        SiteFactKind::Bool => "eq and a boolean",
                        SiteFactKind::Integer => "eq or gte and an integer",
                        SiteFactKind::Id => "eq and an identifier",
                    }));
                }
            }
        };
        Ok(Condition {
            fact: self.fact,
            predicate,
        })
    }
}

impl SiteRequirements {
    pub fn from_toml(
        source: &str,
        resources: &ResourceCatalog,
        configured_fact_types: &HashMap<String, SiteFactKind>,
    ) -> Result<Self, SiteConfigError> {
        let raw: RawRequirements = toml::from_str(source).map_err(SiteConfigError::Parse)?;
        if let Some(efficiencies) = &raw.biome_efficiency {
            if efficiencies.is_empty() {
                return Err(SiteConfigError::EmptyBiomeEfficiency);
            }
            for (biome, &percent) in efficiencies {
                if !BIOMES.iter().any(|(_, id)| id == biome) {
                    return Err(SiteConfigError::UnknownBiome(biome.clone()));
                }
                if !(1..=100).contains(&percent) || biome == "ocean" {
                    return Err(SiteConfigError::InvalidEfficiency {
                        biome: biome.clone(),
                        percent,
                    });
                }
            }
        }
        // A tile has at most one deposit, one biome and one coastal status.
        let mut seen = HashSet::new();
        for condition in &raw.site_requirements {
            let fact = condition.fact.as_str();
            if matches!(fact, TILE_COASTAL | TILE_BIOME | TILE_DEPOSITS) && !seen.insert(fact) {
                return Err(SiteConfigError::DuplicateCondition(fact.to_owned()));
            }
        }

        let conditions = raw
            .site_requirements
            .into_iter()
            .map(|condition| condition.resolve(resources, configured_fact_types))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            conditions,
            biome_efficiency: raw.biome_efficiency,
        })
    }

    pub fn evaluate(
        &self,
        position: Position,
        fact: impl Fn(&str) -> Option<SiteValue>,
    ) -> Result<SiteEvaluation, TerritoryError> {
        let biome_value = fact(TILE_BIOME);
        for condition in &self.conditions {
            let actual = if condition.fact == TILE_BIOME {
                biome_value.clone()
            } else {
                fact(&condition.fact)
            };
            if !condition.predicate.matches(actual) {
                return Err(match (condition.fact.as_str(), &condition.predicate) {
                    (TILE_COASTAL, Predicate::Equals(SiteValue::Bool(true))) => {
                        TerritoryError::CoastRequired(position)
                    }
                    (TILE_BIOME, _) => TerritoryError::ForbiddenBiome(position),
                    (TILE_DEPOSITS, Predicate::Contains(resource)) => {
                        TerritoryError::TileDepositRequired {
                            position,
                            resource: resource.clone(),
                        }
                    }
                    (PROVINCE_DEPOSITS, Predicate::Contains(resource)) => {
                        TerritoryError::ProvinceDepositRequired {
                            position,
                            resource: resource.clone(),
                        }
                    }
                    _ => TerritoryError::SiteRequirementNotMet {
                        position,
                        fact: condition.fact.clone(),
                    },
                });
            }
        }

        let efficiency_percent = match &self.biome_efficiency {
            None => 100,
            Some(efficiencies) => match &biome_value {
                Some(SiteValue::Id(biome)) => *efficiencies
                    .get(biome)
                    .ok_or(TerritoryError::ForbiddenBiome(position))?,
                _ => return Err(TerritoryError::ForbiddenBiome(position)),
            },
        };
        let biome = match biome_value {
            Some(SiteValue::Id(id)) => BIOMES
                .iter()
                .find(|(_, name)| *name == id)
                .map(|(biome, _)| *biome),
            _ => None,
        };
        Ok(SiteEvaluation {
            efficiency_percent,
            biome,
        })
    }

    pub fn requiring_coast(mut self) -> Self {
        self.conditions
            .retain(|condition| condition.fact != TILE_COASTAL);
        self.conditions.push(Condition {
            fact: TILE_COASTAL.to_owned(),
            predicate: Predicate::Equals(SiteValue::Bool(true)),
        });
        self
    }

    pub fn requiring_deposit(mut self, resource: ResourceType) -> Self {
        self.conditions
            .retain(|condition| condition.fact != TILE_DEPOSITS);
        self.conditions.push(Condition {
            fact: TILE_DEPOSITS.to_owned(),
            predicate: Predicate::Contains(resource),
        });
        self
    }

    pub fn requires_coast(&self) -> bool {
        self.conditions.iter().any(|condition| {
            condition.fact == TILE_COASTAL
                && matches!(
                    condition.predicate,
                    Predicate::Equals(SiteValue::Bool(true))
                )
        })
    }

    pub fn tile_deposit(&self) -> Option<&ResourceType> {
        self.conditions.iter().find_map(|condition| {
            match (&*condition.fact, &condition.predicate) {
                (TILE_DEPOSITS, Predicate::Contains(resource)) => Some(resource),
                _ => None,
            }
        })
    }

    pub fn deposits(&self) -> impl Iterator<Item = &ResourceType> {
        self.conditions
            .iter()
            .filter_map(|condition| match &condition.predicate {
                Predicate::Contains(resource) => Some(resource),
                _ => None,
            })
    }
}
