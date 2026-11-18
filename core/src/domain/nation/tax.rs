//! The common tax rate of a nation (gameplay specification §6). Collecting it
//! is a separate step; here it is only policy that nations set.
use serde::{Deserialize, Serialize};
use std::fmt;

/// The common tax rate as a fraction: 0.0 is 0%, 0.1 is 10%, 1.0 is 100%.
/// Serializes as the plain number (`0.15`); values outside 0..=1, NaN and
/// infinities do not parse.
///
/// The field is private and every constructor checks the range, so an invalid
/// rate can never exist in memory and later code does not have to re-check it.
/// Because NaN is excluded, comparing rates is total and `Eq` is sound.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct TaxRate(f64);

impl Eq for TaxRate {}

/// A tax rate outside 0..=1, or not a finite number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidTaxRate(pub f64);

impl fmt::Display for InvalidTaxRate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "tax rate {} is not within 0.0..=1.0", self.0)
    }
}

impl std::error::Error for InvalidTaxRate {}

impl TaxRate {
    /// 10%: the rate every nation starts with.
    pub const BASE: Self = Self(0.1);
    pub const ZERO: Self = Self(0.0);
    pub const FULL: Self = Self(1.0);

    pub fn from_fraction(fraction: f64) -> Result<Self, InvalidTaxRate> {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err(InvalidTaxRate(fraction));
        }
        // `-0.0` lies in the range; store it as `0.0` so it prints and compares plainly.
        Ok(Self(fraction + 0.0))
    }

    /// The rate as a fraction in 0.0..=1.0.
    pub fn fraction(self) -> f64 {
        self.0
    }

    /// The rate in percent, 0.0..=100.0.
    pub fn percent(self) -> f64 {
        self.0 * 100.0
    }
}

impl Default for TaxRate {
    fn default() -> Self {
        Self::BASE
    }
}

impl TryFrom<f64> for TaxRate {
    type Error = InvalidTaxRate;

    fn try_from(fraction: f64) -> Result<Self, Self::Error> {
        Self::from_fraction(fraction)
    }
}

impl From<TaxRate> for f64 {
    fn from(rate: TaxRate) -> Self {
        rate.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_accepts_zero_to_one_and_rejects_everything_else() {
        assert_eq!(TaxRate::from_fraction(0.0), Ok(TaxRate::ZERO));
        assert_eq!(TaxRate::from_fraction(1.0), Ok(TaxRate::FULL));
        assert_eq!(
            TaxRate::from_fraction(-0.0).unwrap().fraction().to_bits(),
            0.0f64.to_bits()
        );
        for invalid in [-0.01, 1.0001, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(TaxRate::from_fraction(invalid).is_err(), "{invalid}");
        }
        assert_eq!(TaxRate::default(), TaxRate::BASE);
        assert!((TaxRate::BASE.percent() - 10.0).abs() < 1e-12);
    }

    #[test]
    fn rate_serializes_as_a_plain_number_and_rejects_invalid_values() {
        assert_eq!(serde_json::to_string(&TaxRate::BASE).unwrap(), "0.1");
        assert_eq!(
            serde_json::from_str::<TaxRate>("0.255").unwrap(),
            TaxRate::from_fraction(0.255).unwrap()
        );
        assert_eq!(serde_json::from_str::<TaxRate>("1").unwrap(), TaxRate::FULL);
        for invalid in ["1.5", "-0.1", "\"0.1\"", "null"] {
            assert!(
                serde_json::from_str::<TaxRate>(invalid).is_err(),
                "{invalid}"
            );
        }
    }
}
