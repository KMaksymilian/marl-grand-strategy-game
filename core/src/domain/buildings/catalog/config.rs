use super::BuildingCatalog;
use crate::domain::buildings::definition::{BuildingDefinition, CycleRecipe, DefinitionError};
use crate::domain::economy::{ResourceCatalog, ResourceType, cost::ProjectCost};
use crate::domain::world::terrain::biome::Biome;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub enum BuildingCatalogError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    Invalid(String),
}

impl fmt::Display for BuildingCatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read building catalog: {error}"),
            Self::Parse(error) => write!(f, "invalid building catalog TOML: {error}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for BuildingCatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    tiers: BTreeMap<String, MapTier>,
    buildings: Vec<RawBuilding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapTier {
    build_turns: u32,
    cycles_per_turn: u32,
    max_workers: u32,
}

type RawMaterials = BTreeMap<String, u32>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBuilding {
    id: String,
    name: String,
    tier: u8,
    cost: RawMaterials,
    cycles_per_turn: Option<u32>,
    #[serde(default)]
    recipes: Vec<RawRecipe>,
    #[serde(default)]
    deposits: Vec<String>,
    #[serde(default)]
    requires_coast: bool,
    biome_efficiencies: Option<BTreeMap<Biome, u8>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecipe {
    #[serde(default)]
    inputs: RawMaterials,
    outputs: RawMaterials,
}

impl BuildingCatalog {
    pub fn load(
        path: impl AsRef<Path>,
        resources: &ResourceCatalog,
    ) -> Result<Self, BuildingCatalogError> {
        let text = std::fs::read_to_string(path).map_err(BuildingCatalogError::Read)?;
        Self::from_toml(&text, resources)
    }

    pub fn from_toml(
        text: &str,
        resources: &ResourceCatalog,
    ) -> Result<Self, BuildingCatalogError> {
        let raw: RawCatalog = toml::from_str(text).map_err(BuildingCatalogError::Parse)?;
        let mut catalog = Self::default();
        for building in raw.buildings {
            building.register(&mut catalog, &raw.tiers, resources)?;
        }
        Ok(catalog)
    }
}

impl RawBuilding {
    fn register(
        self,
        catalog: &mut BuildingCatalog,
        tiers: &BTreeMap<String, MapTier>,
        resources: &ResourceCatalog,
    ) -> Result<(), BuildingCatalogError> {
        let invalid =
            |reason: &str| BuildingCatalogError::Invalid(format!("{}: {reason}", self.id));
        if !valid_building_id(&self.id) || self.name.trim().is_empty() {
            return Err(invalid("requires a snake_case ID and a display name"));
        }
        let tier = tiers
            .get(&self.tier.to_string())
            .filter(|tier| {
                (1..=3).contains(&self.tier)
                    && tier.build_turns > 0
                    && tier.cycles_per_turn > 0
                    && (1..=25).contains(&tier.max_workers)
            })
            .ok_or_else(|| invalid("invalid tier settings"))?;
        let cost = ProjectCost {
            needed_resources: materials(self.cost, resources, &self.id)?
                .into_iter()
                .collect(),
            turn_cost: tier.build_turns,
        };
        let rate = self.cycles_per_turn.unwrap_or(tier.cycles_per_turn);
        let create = |id: String, name: String, recipes: Vec<CycleRecipe>| {
            let report = |error: DefinitionError| invalid(&format!("{error:?}"));
            let mut definition = BuildingDefinition::new(
                id,
                name,
                self.tier,
                tier.max_workers,
                vec![],
                cost.clone(),
            )
            .map_err(report)?
            .with_cycle_production(rate, recipes)
            .map_err(report)?;
            if self.requires_coast {
                definition = definition.requiring_coast();
            }
            if let Some(efficiencies) = &self.biome_efficiencies {
                definition = definition
                    .with_biome_efficiencies(efficiencies.clone())
                    .map_err(report)?;
            }
            Ok(definition)
        };

        if self.deposits.is_empty() {
            let recipes = self
                .recipes
                .into_iter()
                .map(|recipe| {
                    Ok(CycleRecipe {
                        inputs: materials(recipe.inputs, resources, &self.id)?
                            .into_iter()
                            .collect(),
                        outputs: materials(recipe.outputs, resources, &self.id)?
                            .into_iter()
                            .collect(),
                    })
                })
                .collect::<Result<Vec<_>, BuildingCatalogError>>()?;
            insert(
                catalog,
                create(self.id.clone(), self.name.clone(), recipes)?,
            )?;
        } else {
            if !self.recipes.is_empty() {
                return Err(invalid(
                    "deposit variants generate their extraction recipes",
                ));
            }
            for deposit in self.deposits {
                let ore = resources
                    .resolve(&deposit)
                    .ok_or_else(|| invalid(&format!("unknown deposit {deposit:?}")))?;
                let material = resources
                    .get(&ore)
                    .expect("resolved material is registered");
                let id = format!("{}_{}", self.id, material.id());
                let name = format!("{} ({})", self.name, material.name());
                let definition = create(
                    id,
                    name,
                    vec![CycleRecipe {
                        inputs: vec![],
                        outputs: vec![(ore.clone(), 1)],
                    }],
                )?
                .requiring_province_deposit(ore)
                .map_err(|error| invalid(&format!("{error:?}")))?;
                insert(catalog, definition)?;
            }
        }
        Ok(())
    }
}

fn insert(
    catalog: &mut BuildingCatalog,
    definition: BuildingDefinition,
) -> Result<(), BuildingCatalogError> {
    if catalog.get_blueprint(definition.id()).is_some() {
        return Err(BuildingCatalogError::Invalid(format!(
            "duplicate building ID {:?}",
            definition.id()
        )));
    }
    catalog.register(definition);
    Ok(())
}

fn valid_building_id(id: &str) -> bool {
    id.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && id.split('_').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn materials(
    raw: RawMaterials,
    resources: &ResourceCatalog,
    building: &str,
) -> Result<BTreeMap<ResourceType, u32>, BuildingCatalogError> {
    let mut result = BTreeMap::new();
    for (id, amount) in raw {
        let resource = resources.resolve(&id).ok_or_else(|| {
            BuildingCatalogError::Invalid(format!("{building}: unknown material {id:?}"))
        })?;
        if amount == 0 || result.insert(resource, amount).is_some() {
            return Err(BuildingCatalogError::Invalid(format!(
                "{building}: zero quantity or repeated material {id:?}"
            )));
        }
    }
    Ok(result)
}
