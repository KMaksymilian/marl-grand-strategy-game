//! Military (gameplay specification §10 and §11): unit types and rules loaded
//! from `units.toml`. Armies, recruitment, upkeep, movement and battles build
//! on this catalogue.
pub mod unit_catalog;

pub use unit_catalog::{MilitaryRules, UnitCatalog, UnitCatalogError, UnitDefinition, UnitTypeId};
