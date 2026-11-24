//! Validated founding profile shared by newly created settlements.
use super::NeedRegistry;
use crate::domain::economy::{ResourceCatalog, ResourceType, material_units_from_display};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub struct SettlementStart {
    pub(crate) population: u32,
    pub(crate) capacity: u32,
    pub(crate) stock: BTreeMap<ResourceType, u32>,
    pub(crate) needs: Arc<NeedRegistry>,
}

#[derive(Debug)]
pub enum SettlementStartError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    Invalid(String),
}

impl std::fmt::Display for SettlementStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read settlement start: {error}"),
            Self::Parse(error) => write!(f, "invalid settlement start TOML: {error}"),
            Self::Invalid(error) => f.write_str(error),
        }
    }
}

impl std::error::Error for SettlementStartError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStart {
    population: u32,
    warehouse_capacity: u32,
    food_turns: u32,
    materials: BTreeMap<String, u32>,
}

impl SettlementStart {
    pub fn load(
        path: impl AsRef<Path>,
        resources: &ResourceCatalog,
        needs: Arc<NeedRegistry>,
    ) -> Result<Self, SettlementStartError> {
        let source = std::fs::read_to_string(path).map_err(SettlementStartError::Read)?;
        Self::from_toml(&source, resources, needs)
    }

    pub fn from_toml(
        source: &str,
        resources: &ResourceCatalog,
        needs: Arc<NeedRegistry>,
    ) -> Result<Self, SettlementStartError> {
        let raw: RawStart = toml::from_str(source).map_err(SettlementStartError::Parse)?;
        let invalid = |message: &str| SettlementStartError::Invalid(message.to_owned());
        let capacity = material_units_from_display(raw.warehouse_capacity)
            .filter(|&amount| amount > 0)
            .ok_or_else(|| invalid("invalid starting warehouse capacity"))?;
        if raw.population == 0 || raw.food_turns == 0 || needs.food_units_per_person().is_none() {
            return Err(invalid(
                "starting population, food duration and diet must be configured",
            ));
        }

        let mut stock = BTreeMap::new();
        for (name, amount) in raw.materials {
            let resource = resources
                .resolve(&name)
                .ok_or_else(|| invalid(&format!("unknown starting material {name:?}")))?;
            let amount = material_units_from_display(amount)
                .filter(|&amount| amount > 0)
                .ok_or_else(|| invalid(&format!("invalid starting quantity for {name:?}")))?;
            if stock.insert(resource, amount).is_some() {
                return Err(invalid(&format!("duplicate starting material {name:?}")));
            }
        }
        for (resource, one_turn) in needs.starting_food(raw.population) {
            let slot = stock.entry(resource).or_default();
            *slot = one_turn
                .checked_mul(raw.food_turns)
                .and_then(|amount| slot.checked_add(amount))
                .ok_or_else(|| invalid("starting food exceeds supported range"))?;
        }
        if stock.values().map(|&units| u64::from(units)).sum::<u64>() > u64::from(capacity) {
            return Err(invalid("starting stock exceeds warehouse capacity"));
        }
        Ok(Self {
            population: raw.population,
            capacity,
            stock,
            needs,
        })
    }
}
