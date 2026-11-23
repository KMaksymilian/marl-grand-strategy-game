use super::Building;
use crate::domain::spatial::Position;
use crate::domain::world::terrain::biome::Biome;

const BASIS_POINTS_PER_UNIT: u32 = 10_000;

/// A named change to production yield. 100 basis points equal 1%.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionModifier {
    pub source: String,
    pub basis_points: i32,
}

impl ProductionModifier {
    pub fn new(source: impl Into<String>, basis_points: i32) -> Self {
        Self {
            source: source.into(),
            basis_points,
        }
    }
}

pub struct ProductionModifierContext<'a> {
    /// The building already has its actual worker allocation for this turn.
    pub building: &'a Building,
    pub biome: Option<Biome>,
}

/// Supplies one additive yield modifier when its conditions are met.
pub trait ProductionModifierStrategy: std::fmt::Debug + Send + Sync {
    fn modifier(&self, context: &ProductionModifierContext<'_>) -> Option<ProductionModifier>;
}

/// Applies a yield modifier to one building type on a specific biome.
#[derive(Debug)]
pub struct BiomeProductionModifier {
    building_type: String,
    biome: Biome,
    basis_points: i32,
}

impl BiomeProductionModifier {
    pub fn new(building_type: impl Into<String>, biome: Biome, basis_points: i32) -> Self {
        Self {
            building_type: building_type.into(),
            biome,
            basis_points,
        }
    }
}

impl ProductionModifierStrategy for BiomeProductionModifier {
    fn modifier(&self, context: &ProductionModifierContext<'_>) -> Option<ProductionModifier> {
        (context.building.definition().id() == self.building_type
            && context.biome == Some(self.biome))
        .then(|| ProductionModifier::new("biome", self.basis_points))
    }
}

/// Map-independent source of directed movement costs (Strategy boundary).
pub trait TravelCostSource {
    /// Directed cost in the policy's units; `None` means unreachable.
    /// The caller retains buffers; no route or map reference is stored in buildings.
    fn travel_cost(&mut self, from: Position, to: Position) -> Option<u32>;
}

/// Linear output penalty in the movement-cost units used by map A*.
/// Thresholds are supplied by configuration.toml; no balance defaults are embedded here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TravelCostEfficiencyPolicy {
    full_productivity_cost: u32,
    zero_productivity_cost: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTravelCosts;

impl TravelCostEfficiencyPolicy {
    pub fn new(
        full_productivity_cost: u32,
        zero_productivity_cost: u32,
    ) -> Result<Self, InvalidTravelCosts> {
        if full_productivity_cost >= zero_productivity_cost {
            return Err(InvalidTravelCosts);
        }
        Ok(Self {
            full_productivity_cost,
            zero_productivity_cost,
        })
    }

    pub fn full_productivity_cost(self) -> u32 {
        self.full_productivity_cost
    }

    pub fn zero_productivity_cost(self) -> u32 {
        self.zero_productivity_cost
    }

    fn productivity(self, travel_cost: Option<u32>) -> LocationProductivity {
        let Some(cost) = travel_cost else {
            return LocationProductivity::UNREACHABLE;
        };
        if cost <= self.full_productivity_cost {
            LocationProductivity::FULL
        } else if cost >= self.zero_productivity_cost {
            LocationProductivity::ZERO
        } else {
            LocationProductivity {
                numerator: self.zero_productivity_cost - cost,
                denominator: self.zero_productivity_cost - self.full_productivity_cost,
                reachable: true,
            }
        }
    }
}

/// Computes building output multipliers from supplied capital-to-building costs.
/// Owned by BuildingManager; buildings receive only the resulting multiplier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingProductivityCalculator {
    capital: Position,
    policy: TravelCostEfficiencyPolicy,
}

impl BuildingProductivityCalculator {
    pub fn new(capital: Position, policy: TravelCostEfficiencyPolicy) -> Self {
        Self { capital, policy }
    }

    pub fn capital(self) -> Position {
        self.capital
    }

    pub fn policy(self) -> TravelCostEfficiencyPolicy {
        self.policy
    }

    /// Directed route cost from the capital to the building using the supplied source.
    pub fn travel_cost_to(
        self,
        position: Position,
        source: &mut impl TravelCostSource,
    ) -> Option<u32> {
        source.travel_cost(self.capital, position)
    }

    pub fn productivity_at(self, position: Position, source: &mut impl TravelCostSource) -> f64 {
        self.multiplier_at(position, source).as_factor()
    }

    pub(crate) fn multiplier_at(
        self,
        position: Position,
        source: &mut impl TravelCostSource,
    ) -> LocationProductivity {
        self.policy
            .productivity(self.travel_cost_to(position, source))
    }
}

/// Exact internal ratio, independent of resource units. Only valid policies create it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LocationProductivity {
    numerator: u32,
    denominator: u32,
    reachable: bool,
}

impl LocationProductivity {
    pub(crate) const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
        reachable: true,
    };

    const UNREACHABLE: Self = Self {
        reachable: false,
        ..Self::ZERO
    };

    pub(crate) const FULL: Self = Self {
        numerator: 1,
        denominator: 1,
        reachable: true,
    };

    pub(crate) fn as_factor(self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }

    pub(crate) fn scale_output(
        self,
        amount: u32,
        work: u128,
        capacity: u128,
        modifier_basis_points: i128,
    ) -> u64 {
        if !self.reachable {
            return 0;
        }
        // Keep the A* penalty as an exact ratio and add other sources to it.
        let multiplier = i128::from(self.numerator) * i128::from(BASIS_POINTS_PER_UNIT)
            + modifier_basis_points * i128::from(self.denominator);
        let divisor = u128::from(self.denominator) * u128::from(BASIS_POINTS_PER_UNIT);
        let multiplier = multiplier.max(0) as u128;
        let work_output = u128::from(amount) * work;
        // Split the multiplier so the fractional product stays within u128.
        // The work divisor fits in u32, so saturation only affects yields above u64::MAX.
        let whole = work_output.saturating_mul(multiplier / divisor);
        let fractional = work_output * (multiplier % divisor) / divisor;
        let output = whole.saturating_add(fractional) / capacity;
        output.min(u128::from(u64::MAX)) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_must_define_a_nonempty_falloff_interval() {
        for (full, zero) in [(0, 0), (10, 10), (10, 5)] {
            assert_eq!(
                TravelCostEfficiencyPolicy::new(full, zero),
                Err(InvalidTravelCosts)
            );
        }
        assert!(TravelCostEfficiencyPolicy::new(0, u32::MAX).is_ok());
    }

    #[test]
    fn cost_thresholds_and_unreachable_routes_control_productivity() {
        let policy = TravelCostEfficiencyPolicy::new(100, 300).unwrap();
        for (cost, factor) in [(0, 1.0), (100, 1.0), (200, 0.5), (300, 0.0), (400, 0.0)] {
            assert_eq!(policy.productivity(Some(cost)).as_factor(), factor);
        }
        assert_eq!(policy.productivity(None).as_factor(), 0.0);
    }

    #[test]
    fn maximum_costs_and_outputs_do_not_overflow() {
        let policy = TravelCostEfficiencyPolicy::new(0, u32::MAX).unwrap();
        let ratio = policy.productivity(Some(1));
        assert_eq!(
            ratio.scale_output(u32::MAX, u128::from(u32::MAX), u128::from(u32::MAX), 0),
            u64::from(u32::MAX - 1)
        );
        assert_eq!(policy.productivity(Some(u32::MAX)).as_factor(), 0.0);
    }

    #[test]
    fn work_and_location_are_combined_before_rounding_output() {
        let ratio = TravelCostEfficiencyPolicy::new(0, 30)
            .unwrap()
            .productivity(Some(10));
        assert_eq!(ratio.scale_output(3, 1, 2, 0), 1);
    }
}
