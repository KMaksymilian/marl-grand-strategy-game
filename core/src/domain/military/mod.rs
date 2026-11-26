//! Military (gameplay specification §10 and §11): unit types and rules loaded
//! from `units.toml`, armies, recruitment and upkeep. Movement and battles
//! build on these.
pub mod army;
pub mod report;
pub mod unit_catalog;

pub use army::{Army, ArmyRegistry, Recruitment, Unit};
pub use report::{ArmyUpkeepReport, DesertedUnit, MilitaryTurnReport, RecruitsJoined};
pub use unit_catalog::{MilitaryRules, UnitCatalog, UnitCatalogError, UnitDefinition, UnitTypeId};
