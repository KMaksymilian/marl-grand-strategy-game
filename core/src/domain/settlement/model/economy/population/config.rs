use super::{DietShare, FoodNeeds, NeedRegistry, NeedTier};
use crate::domain::economy::{ResourceCatalog, ResourceType};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub enum NeedsConfigError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    Invalid(String),
}

impl fmt::Display for NeedsConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "cannot read settlement needs: {error}"),
            Self::Parse(error) => write!(formatter, "invalid settlement needs TOML: {error}"),
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for NeedsConfigError {
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
struct RawNeeds {
    food_units_per_person: u32,
    food_items: Vec<String>,
    tiers: Vec<RawTier>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTier {
    min_population: u32,
    #[serde(default)]
    materials: BTreeMap<String, u32>,
    diet: Option<Vec<RawDietShare>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDietShare {
    resource: String,
    percent: u32,
}

impl NeedRegistry {
    pub fn load(
        path: impl AsRef<Path>,
        resources: &ResourceCatalog,
    ) -> Result<Self, NeedsConfigError> {
        let text = std::fs::read_to_string(path).map_err(NeedsConfigError::Read)?;
        Self::from_toml(&text, resources)
    }

    pub fn from_toml(text: &str, resources: &ResourceCatalog) -> Result<Self, NeedsConfigError> {
        let mut raw: RawNeeds = toml::from_str(text).map_err(NeedsConfigError::Parse)?;
        if raw.food_units_per_person == 0 || raw.food_items.is_empty() {
            return Err(invalid(
                "food requires a positive per-person rate and a nonempty item list",
            ));
        }
        raw.tiers.sort_unstable_by_key(|tier| tier.min_population);
        if !raw
            .tiers
            .first()
            .is_some_and(|tier| tier.min_population <= 1 && tier.diet.is_some())
        {
            return Err(invalid(
                "the first tier requires a diet and a population threshold of 0 or 1",
            ));
        }
        if raw
            .tiers
            .windows(2)
            .any(|tiers| tiers[0].min_population == tiers[1].min_population)
        {
            return Err(invalid("settlement needs tier thresholds must be unique"));
        }

        let items = raw
            .food_items
            .into_iter()
            .map(|id| {
                let resource = resources
                    .resolve(&id)
                    .ok_or_else(|| invalid(format!("unknown food item {id:?}")))?;
                if resources
                    .get(&resource)
                    .and_then(|item| item.food_class())
                    .is_none()
                {
                    return Err(invalid(format!(
                        "item {id:?} must have a food classification"
                    )));
                }
                Ok(resource)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let allowed: HashSet<_> = items.iter().cloned().collect();
        if allowed.len() != items.len() {
            return Err(invalid("food item list contains repeated resources"));
        }

        let mut tiers = Vec::with_capacity(raw.tiers.len());
        let mut diets = BTreeMap::new();
        let mut cumulative_materials = HashMap::new();
        for tier in raw.tiers {
            let mut needs_per_capita = HashMap::new();
            for (id, amount) in tier.materials {
                let resource = resources
                    .resolve(&id)
                    .ok_or_else(|| invalid(format!("unknown need material {id:?}")))?;
                if amount == 0
                    || allowed.contains(&resource)
                    || needs_per_capita.insert(resource.clone(), amount).is_some()
                {
                    return Err(invalid(format!(
                        "material need {id:?} must be unique, non-dietary, and positive"
                    )));
                }
                let total = cumulative_materials.entry(resource).or_insert(0_u32);
                *total = total.checked_add(amount).ok_or_else(|| {
                    invalid(format!(
                        "per-person demand for material {id:?} exceeds u32 capacity"
                    ))
                })?;
            }
            tiers.push(NeedTier {
                min_population: tier.min_population,
                needs_per_capita,
            });
            if let Some(rows) = tier.diet {
                diets.insert(
                    tier.min_population,
                    parse_diet(tier.min_population, rows, resources, &allowed)?,
                );
            }
        }

        let mut registry = Self::new(tiers);
        registry.food = Some(FoodNeeds {
            units_per_person: raw.food_units_per_person,
            items,
            diets,
        });
        Ok(registry)
    }
}

fn parse_diet(
    threshold: u32,
    rows: Vec<RawDietShare>,
    resources: &ResourceCatalog,
    allowed: &HashSet<ResourceType>,
) -> Result<Vec<DietShare>, NeedsConfigError> {
    let mut seen = HashSet::new();
    let mut total = 0_u32;
    let mut diet = Vec::with_capacity(rows.len());
    for row in rows {
        let resource = resources
            .resolve(&row.resource)
            .filter(|resource| allowed.contains(resource))
            .ok_or_else(|| {
                invalid(format!(
                    "tier {threshold}: diet item {:?} must be listed in food_items",
                    row.resource
                ))
            })?;
        if row.percent == 0 || row.percent > 100 - total || !seen.insert(resource.clone()) {
            return Err(invalid(format!(
                "tier {threshold}: invalid or repeated diet share for {:?}",
                row.resource
            )));
        }
        total += row.percent;
        diet.push(DietShare {
            resource,
            percent: row.percent,
        });
    }
    if total != 100 {
        return Err(invalid(format!(
            "tier {threshold}: diet shares must sum to 100 percent"
        )));
    }
    Ok(diet)
}

fn invalid(message: impl Into<String>) -> NeedsConfigError {
    NeedsConfigError::Invalid(message.into())
}
