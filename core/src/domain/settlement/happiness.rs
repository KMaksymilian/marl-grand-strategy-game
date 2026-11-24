//! Configurable happiness, population change, and gross gold income.
use num_rational::BigRational;
use num_traits::ToPrimitive;
use serde::Deserialize;
use std::{fmt, path::Path};

const MAX_HAPPINESS: u32 = 100;
const BASIS_POINTS_PER_UNIT: u32 = 10_000;

#[derive(Debug, Clone, Copy)]
pub struct HappinessInputs {
    /// The nation's base tax rate, expressed as a fraction.
    pub tax_rate: f32,
    pub monument_happiness: u32,
    pub monument_multiplier: f32,
    pub decree_happiness: f32,
    pub war_weariness: u8,
    pub conquest_turns: u32,
    pub capital_loss_turns: u32,
}

impl Default for HappinessInputs {
    fn default() -> Self {
        Self {
            tax_rate: 0.0,
            monument_happiness: 0,
            monument_multiplier: 1.0,
            decree_happiness: 0.0,
            war_weariness: 0,
            conquest_turns: 0,
            capital_loss_turns: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HappinessOutcome {
    score: BigRational,
    pub production_loss_basis_points: u32,
    pub population_change_basis_points: i32,
}

impl HappinessOutcome {
    /// The clamped score including fractions of a point.
    pub fn score(&self) -> f64 {
        self.score.to_f64().expect("happiness is bounded")
    }

    pub fn floored_score(&self) -> u32 {
        score_floor(&self.score)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DemographicInputs {
    pub opening_population: u32,
    pub unfed_people: u32,
    pub surplus_food: u64,
    pub food_units_per_person: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct DemographicOutcome {
    pub opening_population: u32,
    pub starved: u32,
    pub emigrated: u32,
    pub grew: u32,
    pub closing_population: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct HappinessBand {
    minimum_happiness: u32,
    production_loss_basis_points: u32,
    population_change_basis_points: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHappinessRules {
    base_happiness: u32,
    max_monument_happiness: u32,
    tax_neutral_percentage_points: u32,
    conquest_initial_penalty: u32,
    conquest_recovery_per_turn: u32,
    capital_loss_penalty: u32,
    capital_loss_duration: u32,
    starvation_basis_points: u32,
    gold_per_person_basis_points: u32,
    gold_happiness_baseline: u32,
    bands: Vec<HappinessBand>,
}

/// Validated settlement balance rules.
#[derive(Debug, Clone)]
pub struct HappinessRules(RawHappinessRules);

#[derive(Debug)]
pub enum HappinessRulesError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    Invalid(&'static str),
}

impl fmt::Display for HappinessRulesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "cannot read happiness configuration: {error}"),
            Self::Parse(error) => write!(formatter, "invalid happiness TOML: {error}"),
            Self::Invalid(error) => write!(formatter, "invalid happiness rules: {error}"),
        }
    }
}

impl std::error::Error for HappinessRulesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

impl HappinessRules {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, HappinessRulesError> {
        let source = std::fs::read_to_string(path).map_err(HappinessRulesError::Read)?;
        Self::from_toml(&source)
    }

    pub fn from_toml(source: &str) -> Result<Self, HappinessRulesError> {
        let mut rules: RawHappinessRules =
            toml::from_str(source).map_err(HappinessRulesError::Parse)?;
        rules
            .bands
            .sort_unstable_by_key(|band| band.minimum_happiness);
        if rules.base_happiness > MAX_HAPPINESS {
            return Err(HappinessRulesError::Invalid(
                "base happiness must be in 0–100",
            ));
        }
        if rules.conquest_recovery_per_turn == 0 || rules.gold_happiness_baseline == 0 {
            return Err(HappinessRulesError::Invalid(
                "conquest recovery and the gold happiness baseline must be positive",
            ));
        }
        if rules.starvation_basis_points > BASIS_POINTS_PER_UNIT {
            return Err(HappinessRulesError::Invalid(
                "starvation cannot exceed 100%",
            ));
        }
        if rules
            .bands
            .first()
            .is_none_or(|band| band.minimum_happiness != 0)
            || rules
                .bands
                .windows(2)
                .any(|bands| bands[0].minimum_happiness == bands[1].minimum_happiness)
            || rules.bands.iter().any(|band| {
                band.minimum_happiness > MAX_HAPPINESS
                    || band.production_loss_basis_points > BASIS_POINTS_PER_UNIT
                    || band.population_change_basis_points.unsigned_abs() > BASIS_POINTS_PER_UNIT
            })
        {
            return Err(HappinessRulesError::Invalid(
                "bands must start at zero, have unique bounds in 0–100 and valid rates",
            ));
        }
        Ok(Self(rules))
    }

    pub fn capital_loss_duration(&self) -> u32 {
        self.0.capital_loss_duration
    }

    pub fn conquest_duration(&self) -> u32 {
        self.0
            .conquest_initial_penalty
            .div_ceil(self.0.conquest_recovery_per_turn)
    }

    pub fn evaluate(&self, inputs: HappinessInputs) -> HappinessOutcome {
        let rules = &self.0;
        let elapsed = self
            .conquest_duration()
            .saturating_sub(inputs.conquest_turns);
        let recovered = rules.conquest_recovery_per_turn.saturating_mul(elapsed);
        let conquest = rules.conquest_initial_penalty.saturating_sub(recovered);
        let capital_loss = if inputs.capital_loss_turns > 0 {
            rules.capital_loss_penalty
        } else {
            0
        };
        let monument_points = inputs.monument_happiness.min(rules.max_monument_happiness);
        let score = points(rules.base_happiness) + points(rules.tax_neutral_percentage_points)
            - decimal_factor(inputs.tax_rate, 0) * points(MAX_HAPPINESS)
            + decimal_factor(inputs.monument_multiplier, 1) * points(monument_points)
            + decimal_factor(inputs.decree_happiness, 0)
            - points(u32::from(inputs.war_weariness).min(MAX_HAPPINESS))
            - points(conquest)
            - points(capital_loss);
        let score = score.clamp(points(0), points(MAX_HAPPINESS));
        let floor = score_floor(&score);
        let band = rules
            .bands
            .iter()
            .rev()
            .find(|band| floor >= band.minimum_happiness)
            .expect("the first band starts at zero");
        HappinessOutcome {
            score,
            production_loss_basis_points: band.production_loss_basis_points,
            population_change_basis_points: band.population_change_basis_points,
        }
    }

    pub fn demographics(
        &self,
        happiness: &HappinessOutcome,
        inputs: DemographicInputs,
    ) -> DemographicOutcome {
        let opening = inputs.opening_population;
        let starved = apply_rate(
            inputs.unfed_people.min(opening),
            self.0.starvation_basis_points,
        );
        let survivors = opening - starved;
        let rate = happiness.population_change_basis_points;
        let change = apply_rate(opening, rate.unsigned_abs());
        let (emigrated, grew) = if rate < 0 {
            (change.min(survivors), 0)
        } else {
            let food_room = inputs
                .surplus_food
                .checked_div(u64::from(inputs.food_units_per_person))
                .unwrap_or(0);
            (0, u64::from(change).min(food_room) as u32)
        };
        let grew = grew.min(u32::MAX - survivors);
        DemographicOutcome {
            opening_population: opening,
            starved,
            emigrated,
            grew,
            closing_population: survivors - emigrated + grew,
        }
    }

    /// Uses the full score and floors gold only after multiplying.
    pub fn gross_gold_income(&self, opening_population: u32, happiness: &HappinessOutcome) -> u64 {
        let income = &happiness.score
            * points(opening_population)
            * points(self.0.gold_per_person_basis_points)
            / points(self.0.gold_happiness_baseline)
            / points(BASIS_POINTS_PER_UNIT);
        income.to_integer().to_u64().unwrap_or(u64::MAX)
    }
}

fn points(value: u32) -> BigRational {
    BigRational::from_integer(value.into())
}

fn score_floor(score: &BigRational) -> u32 {
    score.to_integer().to_u32().expect("happiness is bounded")
}

fn apply_rate(value: u32, basis_points: u32) -> u32 {
    (u64::from(value) * u64::from(basis_points) / u64::from(BASIS_POINTS_PER_UNIT)) as u32
}

/// Use the f32's round-trippable decimal value, so a nominal 10% tax cancels
/// exactly 10 points. Binary floating-point noise could select the wrong band.
fn decimal_factor(value: f32, fallback: u32) -> BigRational {
    if !value.is_finite() {
        return points(fallback);
    }
    let mut digits = value.to_string();
    let Some(separator) = digits.find('.') else {
        return BigRational::from_integer(digits.parse().expect("finite decimal integer"));
    };
    digits.remove(separator);
    let denominator = format!("1{}", "0".repeat(digits.len() - separator));
    BigRational::new(
        digits.parse().expect("finite decimal numerator"),
        denominator.parse().expect("positive decimal denominator"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = include_str!("../../../../settlement_happiness.toml");

    fn rules() -> HappinessRules {
        HappinessRules::from_toml(BUNDLED).unwrap()
    }

    fn score(rules: &HappinessRules, score: f32) -> HappinessOutcome {
        rules.evaluate(HappinessInputs {
            tax_rate: 0.1,
            decree_happiness: score - 50.0,
            ..HappinessInputs::default()
        })
    }

    #[test]
    fn exact_tax_and_floor_choose_the_configured_bands() {
        let rules = rules();
        for (input, floor, production_loss, population_change) in [
            (-10.0, 0, 2_000, -200),
            (29.999, 29, 2_000, -200),
            (30.0, 30, 0, -100),
            (49.999, 49, 0, -100),
            (50.0, 50, 0, 0),
            (54.999, 54, 0, 0),
            (55.0, 55, 0, 100),
            (79.999, 79, 0, 100),
            (80.0, 80, 0, 200),
            (120.0, 100, 0, 200),
        ] {
            let outcome = score(&rules, input);
            assert_eq!(outcome.floored_score(), floor);
            assert_eq!(outcome.production_loss_basis_points, production_loss);
            assert_eq!(outcome.population_change_basis_points, population_change);
        }
    }

    #[test]
    fn applies_all_happiness_sources_and_decaying_conquest() {
        let rules = rules();
        assert_eq!(rules.conquest_duration(), 4);
        assert_eq!(rules.capital_loss_duration(), 10);
        let mut inputs = HappinessInputs {
            tax_rate: 0.2,
            monument_happiness: 25,
            monument_multiplier: 1.5,
            decree_happiness: 4.5,
            war_weariness: 8,
            conquest_turns: 4,
            capital_loss_turns: 1,
        };
        assert_eq!(rules.evaluate(inputs).score(), 36.5);
        inputs.conquest_turns -= 1;
        assert_eq!(rules.evaluate(inputs).score(), 41.5);
        inputs.capital_loss_turns = 0;
        inputs.conquest_turns = 0;
        assert_eq!(rules.evaluate(inputs).score(), 66.5);
    }

    #[test]
    fn growth_is_capped_by_food_and_starvation_uses_the_unfed_count() {
        let rules = rules();
        let inputs = DemographicInputs {
            opening_population: 1000,
            unfed_people: 109,
            surplus_food: 19,
            food_units_per_person: 2,
        };
        assert_eq!(
            rules.demographics(&score(&rules, 55.0), inputs),
            DemographicOutcome {
                opening_population: 1000,
                starved: 10,
                emigrated: 0,
                grew: 9,
                closing_population: 999,
            }
        );
        assert_eq!(
            rules.demographics(&score(&rules, 29.0), inputs),
            DemographicOutcome {
                opening_population: 1000,
                starved: 10,
                emigrated: 20,
                grew: 0,
                closing_population: 970,
            }
        );
        assert_eq!(
            rules
                .demographics(
                    &score(&rules, 80.0),
                    DemographicInputs {
                        surplus_food: 100,
                        ..inputs
                    }
                )
                .grew,
            20
        );
        assert_eq!(
            rules
                .demographics(
                    &score(&rules, 80.0),
                    DemographicInputs {
                        food_units_per_person: 0,
                        ..inputs
                    }
                )
                .grew,
            0
        );
    }

    #[test]
    fn income_uses_opening_population_and_keeps_fractional_happiness_until_gold_floor() {
        let rules = rules();
        assert_eq!(rules.gross_gold_income(1000, &score(&rules, 50.0)), 1000);
        let fraction = rules.evaluate(HappinessInputs {
            tax_rate: 0.1,
            decree_happiness: 0.075,
            ..HappinessInputs::default()
        });
        assert_eq!(rules.gross_gold_income(1000, &fraction), 1001);
        assert_eq!(rules.gross_gold_income(1000, &score(&rules, -10.0)), 0);
        assert_eq!(
            rules.gross_gold_income(u32::MAX, &score(&rules, 110.0)),
            u64::from(u32::MAX) * 2
        );
    }

    #[test]
    fn validates_rates_and_uses_configured_durations_and_gold() {
        for (from, to) in [
            ("minimum_happiness = 30", "minimum_happiness = 0"),
            ("minimum_happiness = 80", "minimum_happiness = 101"),
            ("minimum_happiness = 0", "minimum_happiness = 1"),
            (
                "starvation_basis_points = 1000",
                "starvation_basis_points = 10001",
            ),
            (
                "gold_happiness_baseline = 50",
                "gold_happiness_baseline = 0",
            ),
            (
                "conquest_recovery_per_turn = 5",
                "conquest_recovery_per_turn = 0",
            ),
        ] {
            assert!(HappinessRules::from_toml(&BUNDLED.replace(from, to)).is_err());
        }
        let custom = HappinessRules::from_toml(
            &BUNDLED
                .replace(
                    "conquest_initial_penalty = 20",
                    "conquest_initial_penalty = 23",
                )
                .replace("capital_loss_duration = 10", "capital_loss_duration = 7")
                .replace(
                    "gold_per_person_basis_points = 10000",
                    "gold_per_person_basis_points = 15000",
                ),
        )
        .unwrap();
        assert_eq!(custom.conquest_duration(), 5);
        assert_eq!(custom.capital_loss_duration(), 7);
        for (turns, expected) in [(5, 27), (4, 32), (1, 47), (0, 50)] {
            let outcome = custom.evaluate(HappinessInputs {
                tax_rate: 0.1,
                conquest_turns: turns,
                ..HappinessInputs::default()
            });
            assert_eq!(outcome.floored_score(), expected);
        }
        assert_eq!(custom.gross_gold_income(1000, &score(&custom, 50.0)), 1500);
    }
}
