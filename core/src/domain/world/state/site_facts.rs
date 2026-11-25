//! Configured tile properties are loaded atomically and resolved alongside live map facts.
use super::World;
use crate::domain::spatial::Position;
use crate::domain::spatial::site::{
    SiteFactKind, SiteValue, TILE_BIOME, TILE_COASTAL, TILE_DEPOSITS,
};
use crate::domain::world::map::SiteFactsConfigError;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, hash_map::Entry};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteFactsConfig {
    #[serde(default)]
    fact_types: BTreeMap<String, SiteFactKind>,
    #[serde(default)]
    tiles: Vec<TileFacts>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TileFacts {
    x: i32,
    y: i32,
    facts: BTreeMap<String, toml::Value>,
}

impl World {
    /// Replaces the set of scenario-defined facts after validating every declaration and tile.
    /// Load the world facts before parsing building requirements that reference them.
    pub fn load_site_facts_toml(&mut self, source: &str) -> Result<(), SiteFactsConfigError> {
        let config: SiteFactsConfig =
            toml::from_str(source).map_err(SiteFactsConfigError::Parse)?;
        for name in config.fact_types.keys() {
            let valid = name.strip_prefix("tile.").is_some_and(|id| {
                id.split('_').all(|part| {
                    part.as_bytes().split_first().is_some_and(|(first, rest)| {
                        first.is_ascii_lowercase()
                            && rest
                                .iter()
                                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                    })
                })
            });
            if !valid || matches!(name.as_str(), TILE_COASTAL | TILE_BIOME | TILE_DEPOSITS) {
                return Err(SiteFactsConfigError::InvalidFactName(name.clone()));
            }
        }

        let mut site_facts = HashMap::with_capacity(config.tiles.len());
        for TileFacts { x, y, facts } in config.tiles {
            let position = Position { x, y };
            if self.map.tile_index(position).is_none() {
                return Err(SiteFactsConfigError::InvalidPosition(position));
            }
            let Entry::Vacant(tile) = site_facts.entry(position) else {
                return Err(SiteFactsConfigError::DuplicateTile(position));
            };
            let mut values = HashMap::with_capacity(facts.len());
            for (fact, raw_value) in facts {
                let Some(&expected) = config.fact_types.get(&fact) else {
                    return Err(SiteFactsConfigError::UnknownFact(fact));
                };
                let value = match (expected, raw_value) {
                    (SiteFactKind::Bool, toml::Value::Boolean(value)) => SiteValue::Bool(value),
                    (SiteFactKind::Integer, toml::Value::Integer(value)) => {
                        SiteValue::Integer(value)
                    }
                    (SiteFactKind::Id, toml::Value::String(value)) => SiteValue::Id(value),
                    _ => return Err(SiteFactsConfigError::InvalidValue { fact, expected }),
                };
                values.insert(fact, value);
            }
            tile.insert(values);
        }

        self.map.site_fact_types = config.fact_types.into_iter().collect();
        self.map.site_facts = site_facts;
        Ok(())
    }

    /// Types declared by the current scenario for additional map properties.
    pub fn site_fact_types(&self) -> &HashMap<String, SiteFactKind> {
        &self.map.site_fact_types
    }
}
