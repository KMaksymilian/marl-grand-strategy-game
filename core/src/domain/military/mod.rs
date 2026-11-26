//! Military (gameplay specification §10 and §11): unit types and rules loaded
//! from `units.toml`, armies, recruitment, upkeep, movement and battles.
pub mod army;
pub mod battle;
pub mod report;
pub mod unit_catalog;

pub use army::{Army, ArmyRegistry, Garrison, Recruitment, Unit};
pub use report::{
    ArmyMarch, ArmyUpkeepReport, ArmyWithdrawal, BattleReport, BattleSide, DesertedUnit,
    MarchOutcome, MilitaryTurnReport, RecruitsJoined,
};
pub use unit_catalog::{MilitaryRules, UnitCatalog, UnitCatalogError, UnitDefinition, UnitTypeId};
