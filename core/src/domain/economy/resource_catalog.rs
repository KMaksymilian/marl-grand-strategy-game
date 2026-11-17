use super::ResourceType;
use serde::Deserialize;
use serde::de::value::{Error as DeserializeError, StrDeserializer};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoodClass {
    Raw,
    Processed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceCategory {
    RawFood,
    ProcessedFood,
    ConstructionMaterial,
    Metal,
    Defense,
    CraftedGood,
    Beverage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceDefinition {
    id: String,
    name: String,
    tier: u8,
    category: ResourceCategory,
    #[serde(default)]
    aliases: Vec<String>,
}

impl ResourceDefinition {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn tier(&self) -> u8 {
        self.tier
    }

    pub fn category(&self) -> ResourceCategory {
        self.category
    }

    pub fn food_class(&self) -> Option<FoodClass> {
        match self.category {
            ResourceCategory::RawFood => Some(FoodClass::Raw),
            ResourceCategory::ProcessedFood => Some(FoodClass::Processed),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ResourceCatalogError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    UnknownMaterial(String),
    LabourIsNotMaterial,
    DuplicateMaterial(ResourceType),
    MissingMaterial(ResourceType),
    InvalidTier { resource: ResourceType, tier: u8 },
    EmptyName(ResourceType),
    InvalidAlias(String),
    DuplicateIdentifier(String),
    InvalidCoalAlias(&'static str),
}

impl fmt::Display for ResourceCatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read resource catalogue: {error}"),
            Self::Parse(error) => write!(f, "invalid resource catalogue TOML: {error}"),
            Self::UnknownMaterial(id) => write!(f, "unknown material {id:?}"),
            Self::LabourIsNotMaterial => write!(f, "labour cannot be registered as a material"),
            Self::DuplicateMaterial(resource) => write!(f, "duplicate material {resource:?}"),
            Self::MissingMaterial(resource) => write!(f, "missing material {resource:?}"),
            Self::InvalidTier { resource, tier } => {
                write!(
                    f,
                    "material {resource:?} has tier {tier}; expected 1 through 3"
                )
            }
            Self::EmptyName(resource) => write!(f, "material {resource:?} has an empty name"),
            Self::InvalidAlias(id) => write!(f, "invalid material alias {id:?}"),
            Self::DuplicateIdentifier(id) => write!(f, "duplicate material identifier {id:?}"),
            Self::InvalidCoalAlias(id) => write!(f, "material alias {id:?} must resolve to coal"),
        }
    }
}

impl std::error::Error for ResourceCatalogError {
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
    resources: Vec<ResourceDefinition>,
}

/// Material metadata and default warehouse priorities loaded from content.
#[derive(Debug, Clone)]
pub struct ResourceCatalog {
    definitions: HashMap<ResourceType, ResourceDefinition>,
    identifiers: HashMap<String, ResourceType>,
    materials: Vec<ResourceType>,
}

impl ResourceCatalog {
    pub fn standard() -> &'static Self {
        static CATALOG: OnceLock<ResourceCatalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            Self::from_toml(include_str!("../../../../resources.toml"))
                .expect("the bundled resource catalogue must be valid")
        })
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, ResourceCatalogError> {
        let source = std::fs::read_to_string(path).map_err(ResourceCatalogError::Read)?;
        Self::from_toml(&source)
    }

    pub fn from_toml(source: &str) -> Result<Self, ResourceCatalogError> {
        let raw: RawCatalog = toml::from_str(source).map_err(ResourceCatalogError::Parse)?;
        let mut catalog = Self {
            definitions: HashMap::new(),
            identifiers: HashMap::new(),
            materials: Vec::new(),
        };
        for definition in raw.resources {
            catalog.insert(definition)?;
        }
        for &resource in ResourceType::MATERIALS {
            if !catalog.definitions.contains_key(&resource) {
                return Err(ResourceCatalogError::MissingMaterial(resource));
            }
        }
        for alias in ["charcoal", "mined_coal"] {
            if catalog.resolve(alias) != Some(ResourceType::Coal) {
                return Err(ResourceCatalogError::InvalidCoalAlias(alias));
            }
        }
        Ok(catalog)
    }

    /// Look up a canonical identifier or an alias written in content.
    pub fn resolve(&self, id: &str) -> Option<ResourceType> {
        self.identifiers.get(id).copied()
    }

    pub fn get(&self, resource: ResourceType) -> Option<&ResourceDefinition> {
        self.definitions.get(&resource)
    }

    /// Materials in their configured default warehouse order.
    pub fn materials(&self) -> &[ResourceType] {
        &self.materials
    }

    fn insert(&mut self, definition: ResourceDefinition) -> Result<(), ResourceCatalogError> {
        let variant_name: String = definition
            .id
            .split('_')
            .map(|word| {
                let (first, rest) = word.split_at_checked(1)?;
                if !word.bytes().all(|byte| byte.is_ascii_lowercase()) {
                    return None;
                }
                Some(first.to_ascii_uppercase() + rest)
            })
            .collect::<Option<_>>()
            .ok_or_else(|| ResourceCatalogError::UnknownMaterial(definition.id.clone()))?;
        let resource =
            ResourceType::deserialize(StrDeserializer::<DeserializeError>::new(&variant_name))
                .map_err(|_| ResourceCatalogError::UnknownMaterial(definition.id.clone()))?;
        if resource == ResourceType::Labour {
            return Err(ResourceCatalogError::LabourIsNotMaterial);
        }
        if self.definitions.contains_key(&resource) {
            return Err(ResourceCatalogError::DuplicateMaterial(resource));
        }
        if !(1..=3).contains(&definition.tier) {
            return Err(ResourceCatalogError::InvalidTier {
                resource,
                tier: definition.tier,
            });
        }
        if definition.name.trim().is_empty() {
            return Err(ResourceCatalogError::EmptyName(resource));
        }
        self.register_id(&definition.id, resource)?;
        for alias in &definition.aliases {
            if alias.is_empty()
                || alias == "labour"
                || !alias
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            {
                return Err(ResourceCatalogError::InvalidAlias(alias.clone()));
            }
            self.register_id(alias, resource)?;
        }
        self.definitions.insert(resource, definition);
        self.materials.push(resource);
        Ok(())
    }

    fn register_id(
        &mut self,
        id: &str,
        resource: ResourceType,
    ) -> Result<(), ResourceCatalogError> {
        if self.identifiers.insert(id.to_owned(), resource).is_some() {
            return Err(ResourceCatalogError::DuplicateIdentifier(id.to_owned()));
        }
        Ok(())
    }
}
