use super::BuildingCatalog;
use crate::domain::buildings::definition::{BuildingDefinition, CycleRecipe, DefinitionError};
use crate::domain::buildings::passive::{BuildingUpgradeRule, PassiveBuildingRules};
use crate::domain::economy::{
    MATERIAL_UNITS_PER_DISPLAY_UNIT, ResourceCatalog, ResourceType, cost::ProjectCost,
    material_units_from_display,
};
use crate::domain::spatial::site::{RawCondition, SiteFactKind, SiteRequirements};
use crate::domain::world::terrain::biome::Biome;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
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
    #[serde(default)]
    tiers: BTreeMap<String, MapTier>,
    #[serde(default)]
    buildings: Vec<RawBuilding>,
    #[serde(default)]
    special_buildings: Vec<RawSpecialBuilding>,
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
struct RawSpecialBuilding {
    id: String,
    name: String,
    cost: RawMaterials,
    build_turns: u32,
    rules: PassiveBuildingRules,
}

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
    #[serde(default)]
    site_requirements: Vec<RawCondition>,
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
        Self::from_toml_with_site_facts(text, resources, &HashMap::new())
    }

    pub fn from_toml_with_site_facts(
        text: &str,
        resources: &ResourceCatalog,
        site_fact_types: &HashMap<String, SiteFactKind>,
    ) -> Result<Self, BuildingCatalogError> {
        let raw: RawCatalog = toml::from_str(text).map_err(BuildingCatalogError::Parse)?;
        let mut catalog = Self::default();
        for building in raw.buildings {
            building.register(&mut catalog, &raw.tiers, resources, site_fact_types)?;
        }
        for building in raw.special_buildings {
            building.register(&mut catalog, resources)?;
        }
        validate_special_rules(&catalog)?;
        Ok(catalog)
    }
}

impl RawSpecialBuilding {
    fn register(
        self,
        catalog: &mut BuildingCatalog,
        resources: &ResourceCatalog,
    ) -> Result<(), BuildingCatalogError> {
        if !valid_building_id(&self.id)
            || self.name.trim().is_empty()
            || self.build_turns == 0
            || self
                .rules
                .limit
                .as_ref()
                .is_some_and(|limit| limit.maximum == 0 || limit.group.trim().is_empty())
        {
            return Err(BuildingCatalogError::Invalid(format!(
                "{}: invalid special building settings",
                self.id
            )));
        }
        let cost = ProjectCost {
            needed_resources: materials(self.cost, resources, &self.id)?
                .into_iter()
                .collect(),
            turn_cost: self.build_turns,
        };
        let mut rules = self.rules;
        let effects = &mut rules.effects;
        effects.warehouse_capacity = material_units_from_display(effects.warehouse_capacity)
            .ok_or_else(|| {
                BuildingCatalogError::Invalid(format!(
                    "{}: warehouse capacity exceeds the material unit limit",
                    self.id
                ))
            })?;
        effects.trade = effects.trade.into_material_units().ok_or_else(|| {
            BuildingCatalogError::Invalid(format!(
                "{}: trade capacity exceeds the material unit limit",
                self.id
            ))
        })?;
        let definition = BuildingDefinition::new(&self.id, self.name, 0, 0, vec![], cost)
            .map_err(|error| BuildingCatalogError::Invalid(format!("{}: {error:?}", self.id)))?
            .with_passive_rules(rules);
        insert(catalog, definition)
    }
}

fn validate_special_rules(catalog: &BuildingCatalog) -> Result<(), BuildingCatalogError> {
    let mut group_limits = BTreeMap::new();
    for id in catalog.ids() {
        let definition = catalog.get_blueprint(id).expect("registered building");
        if let Some(limit) = &definition.passive_rules().limit
            && group_limits
                .insert(limit.group.clone(), limit.maximum)
                .is_some_and(|previous| previous != limit.maximum)
        {
            return Err(BuildingCatalogError::Invalid(format!(
                "{id}: inconsistent placement cap for group {:?}",
                limit.group
            )));
        }
        let mut path = std::collections::HashSet::new();
        let mut current = definition;
        while let BuildingUpgradeRule::From(previous) = &current.passive_rules().upgrade {
            if !path.insert(current.id().to_owned()) {
                return Err(BuildingCatalogError::Invalid(format!(
                    "{id}: cyclic upgrade chain"
                )));
            }
            current = catalog.get_blueprint(previous).ok_or_else(|| {
                BuildingCatalogError::Invalid(format!("{id}: unknown upgrade source {previous:?}"))
            })?;
        }
    }
    Ok(())
}

impl RawBuilding {
    fn register(
        self,
        catalog: &mut BuildingCatalog,
        tiers: &BTreeMap<String, MapTier>,
        resources: &ResourceCatalog,
        site_fact_types: &HashMap<String, SiteFactKind>,
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
        let site =
            SiteRequirements::from_conditions(self.site_requirements, resources, site_fact_types)
                .map_err(|error| invalid(&error.to_string()))?;
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
            .with_site_requirements(site.clone())
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
                        outputs: vec![(ore.clone(), MATERIAL_UNITS_PER_DISPLAY_UNIT)],
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
        let amount = material_units_from_display(amount).ok_or_else(|| {
            BuildingCatalogError::Invalid(format!(
                "{building}: quantity for {id:?} exceeds the material unit limit"
            ))
        })?;
        if amount == 0 || result.insert(resource, amount).is_some() {
            return Err(BuildingCatalogError::Invalid(format!(
                "{building}: zero quantity or repeated material {id:?}"
            )));
        }
    }
    Ok(result)
}
