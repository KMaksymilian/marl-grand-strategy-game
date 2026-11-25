use crate::buildings::passive::TradeLimitBonuses;
use crate::buildings::productivity::{InvalidTravelCosts, TravelCostEfficiencyPolicy};
use crate::domain::economy::material_units_from_display;
use serde::Deserialize;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct Configuration {
    pub economy: EconomyConfiguration,
}

#[derive(Debug, Clone, Copy)]
pub struct EconomyConfiguration {
    pub building_productivity: TravelCostEfficiencyPolicy,
    pub trade_limits: Option<TradeLimitBonuses>,
    pub national_stock_capacity_per_settlement: Option<u32>,
}

#[derive(Debug)]
pub enum ConfigurationError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    InvalidTravelCosts(InvalidTravelCosts),
    TradeCapacityOverflow,
    NationalStockCapacityOverflow,
}

impl fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read configuration: {error}"),
            Self::Parse(error) => write!(f, "invalid configuration TOML: {error}"),
            Self::InvalidTravelCosts(_) => write!(
                f,
                "economy.building_productivity.full_productivity_cost must be smaller than zero_productivity_cost"
            ),
            Self::TradeCapacityOverflow => {
                write!(
                    f,
                    "economy.trade_limits material capacity exceeds the material unit limit"
                )
            }
            Self::NationalStockCapacityOverflow => write!(
                f,
                "economy.national_stock_capacity_per_settlement exceeds the material unit limit"
            ),
        }
    }
}

impl std::error::Error for ConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::InvalidTravelCosts(_)
            | Self::TradeCapacityOverflow
            | Self::NationalStockCapacityOverflow => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfiguration {
    economy: RawEconomy,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEconomy {
    building_productivity: RawBuildingProductivity,
    trade_limits: Option<TradeLimitBonuses>,
    national_stock_capacity_per_settlement: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBuildingProductivity {
    full_productivity_cost: u32,
    zero_productivity_cost: u32,
}

impl Configuration {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigurationError> {
        let source = std::fs::read_to_string(path).map_err(ConfigurationError::Read)?;
        Self::from_toml(&source)
    }

    pub fn from_toml(source: &str) -> Result<Self, ConfigurationError> {
        let raw: RawConfiguration = toml::from_str(source).map_err(ConfigurationError::Parse)?;
        let settings = raw.economy.building_productivity;
        let policy = TravelCostEfficiencyPolicy::new(
            settings.full_productivity_cost,
            settings.zero_productivity_cost,
        )
        .map_err(ConfigurationError::InvalidTravelCosts)?;
        Ok(Self {
            economy: EconomyConfiguration {
                building_productivity: policy,
                trade_limits: raw
                    .economy
                    .trade_limits
                    .map(|limits| {
                        limits
                            .into_material_units()
                            .ok_or(ConfigurationError::TradeCapacityOverflow)
                    })
                    .transpose()?,
                national_stock_capacity_per_settlement: raw
                    .economy
                    .national_stock_capacity_per_settlement
                    .map(|capacity| {
                        material_units_from_display(capacity)
                            .ok_or(ConfigurationError::NationalStockCapacityOverflow)
                    })
                    .transpose()?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_configuration_is_valid() {
        let config = Configuration::from_toml(include_str!("../../configuration.toml")).unwrap();
        let policy = config.economy.building_productivity;
        assert!(policy.full_productivity_cost() < policy.zero_productivity_cost());
        let limits = config.economy.trade_limits.unwrap();
        assert_eq!((limits.caravan_slots, limits.offer_slots), (2, 4));
        assert_eq!(
            (limits.caravan_cargo, limits.open_offer_units),
            (40_000, 80_000)
        );
        assert_eq!(
            config.economy.national_stock_capacity_per_settlement,
            Some(50_000)
        );
    }

    #[test]
    fn trade_capacity_must_fit_the_material_unit_scale() {
        let source = include_str!("../../configuration.toml");
        for (from, to) in [
            ("caravan_cargo = 400", "caravan_cargo = 42949673"),
            ("open_offer_units = 800", "open_offer_units = 42949673"),
        ] {
            assert!(matches!(
                Configuration::from_toml(&source.replace(from, to)),
                Err(ConfigurationError::TradeCapacityOverflow)
            ));
        }
    }

    #[test]
    fn national_stock_capacity_must_fit_the_material_unit_scale() {
        let source = include_str!("../../configuration.toml").replace(
            "national_stock_capacity_per_settlement = 500",
            "national_stock_capacity_per_settlement = 42949673",
        );
        assert!(matches!(
            Configuration::from_toml(&source),
            Err(ConfigurationError::NationalStockCapacityOverflow)
        ));
    }

    #[test]
    fn thresholds_come_from_toml_without_hidden_defaults() {
        let config = Configuration::from_toml(
            "[economy.building_productivity]\nfull_productivity_cost = 17\nzero_productivity_cost = 81",
        ).unwrap();
        assert_eq!(
            config
                .economy
                .building_productivity
                .full_productivity_cost(),
            17
        );
        assert_eq!(
            config
                .economy
                .building_productivity
                .zero_productivity_cost(),
            81
        );
        for source in [
            "",
            "[economy.building_productivity]\nfull_productivity_cost = 17",
            "[economy.building_productivity]\nfull_productivity_cost = -1\nzero_productivity_cost = 81",
            "[economy.building_productivity]\nfull_productivity_cost = 17\nzero_productivity_cost = 81\nunknown = 1",
        ] {
            assert!(Configuration::from_toml(source).is_err());
        }
    }

    #[test]
    fn equal_or_inverted_thresholds_are_rejected() {
        for full in [20, 30] {
            let source = format!(
                "[economy.building_productivity]\nfull_productivity_cost = {full}\nzero_productivity_cost = 20"
            );
            assert!(matches!(
                Configuration::from_toml(&source),
                Err(ConfigurationError::InvalidTravelCosts(_))
            ));
        }
    }

    #[test]
    fn missing_configuration_file_is_reported() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("missing-configuration-for-test.toml");
        assert!(matches!(
            Configuration::load(path),
            Err(ConfigurationError::Read(_))
        ));
    }
}
