use super::{ResourceId, ResourceType};
use serde::Deserialize;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU32;
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
    max_vein_tiles: Option<NonZeroU32>,
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

    pub fn max_vein_tiles(&self) -> Option<NonZeroU32> {
        self.max_vein_tiles
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
    InvalidIdentifier(String),
    LabourIsNotMaterial,
    DuplicateMaterial(ResourceType),
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
            Self::InvalidIdentifier(id) => write!(f, "invalid material identifier {id:?}"),
            Self::LabourIsNotMaterial => write!(f, "labour cannot be registered as a material"),
            Self::DuplicateMaterial(resource) => write!(f, "duplicate material {resource:?}"),
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
    deposits: Option<DepositLimits>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DepositLimits {
    pub max_vein_tiles: NonZeroU32,
    pub max_veins: u32,
}

/// Material metadata and default warehouse priorities loaded from content.
#[derive(Debug, Clone)]
pub struct ResourceCatalog {
    definitions: HashMap<ResourceType, ResourceDefinition>,
    identifiers: HashMap<String, ResourceType>,
    materials: Vec<ResourceType>,
    deposit_limits: Option<DepositLimits>,
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
            deposit_limits: raw.deposits,
        };
        for definition in raw.resources {
            catalog.insert(definition)?;
        }
        let coal = ResourceType::Material(
            ResourceId::from_content_id("coal").expect("coal is a valid content identifier"),
        );
        let has_coal = catalog.definitions.contains_key(&coal);
        for alias in ["charcoal", "mined_coal"] {
            if catalog.resolve(alias).as_ref() != has_coal.then_some(&coal) {
                return Err(ResourceCatalogError::InvalidCoalAlias(alias));
            }
        }
        Ok(catalog)
    }

    /// Look up a canonical identifier or an alias written in content.
    pub fn resolve(&self, id: &str) -> Option<ResourceType> {
        self.identifiers.get(id).cloned()
    }

    pub fn get(&self, resource: impl Borrow<ResourceType>) -> Option<&ResourceDefinition> {
        self.definitions.get(resource.borrow())
    }

    /// Materials in their configured default warehouse order.
    pub fn materials(&self) -> &[ResourceType] {
        &self.materials
    }

    pub(crate) fn deposit_limits(&self) -> DepositLimits {
        self.deposit_limits.unwrap_or_else(|| {
            Self::standard()
                .deposit_limits
                .expect("the bundled resource catalogue defines deposit limits")
        })
    }

    fn insert(&mut self, definition: ResourceDefinition) -> Result<(), ResourceCatalogError> {
        if definition.id == "labour" {
            return Err(ResourceCatalogError::LabourIsNotMaterial);
        }
        let id = ResourceId::from_content_id(&definition.id)
            .ok_or_else(|| ResourceCatalogError::InvalidIdentifier(definition.id.clone()))?;
        let resource = ResourceType::Material(id);
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
        self.register_id(&definition.id, &resource)?;
        for alias in &definition.aliases {
            if ResourceId::from_content_id(alias).is_none() {
                return Err(ResourceCatalogError::InvalidAlias(alias.clone()));
            }
            self.register_id(alias, &resource)?;
        }
        self.materials.push(resource.clone());
        self.definitions.insert(resource, definition);
        Ok(())
    }

    fn register_id(
        &mut self,
        id: &str,
        resource: &ResourceType,
    ) -> Result<(), ResourceCatalogError> {
        if self
            .identifiers
            .insert(id.to_owned(), resource.clone())
            .is_some()
        {
            return Err(ResourceCatalogError::DuplicateIdentifier(id.to_owned()));
        }
        Ok(())
    }
}
