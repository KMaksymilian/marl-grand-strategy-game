use super::Building;
use crate::domain::economy::ResourceType;
use crate::domain::spatial::Position;
use crate::domain::world::terrain::biome::Biome;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

const BASIS_POINTS_PER_UNIT: u32 = 10_000;

/// An exact production multiplier. A factor of 1 means unchanged yield.
pub type ProductionFactor = num_rational::BigRational;

/// A named operation on the current production factor.
#[derive(Clone)]
pub struct ProductionModifier {
    pub source: String,
    operation: Arc<dyn Fn(ProductionFactor) -> ProductionFactor + Send + Sync>,
}

impl ProductionModifier {
    /// Adds points to the current factor. 100 basis points equal 1 percentage point.
    pub fn new(source: impl Into<String>, basis_points: i32) -> Self {
        let change = ProductionFactor::new(basis_points.into(), BASIS_POINTS_PER_UNIT.into());
        Self::with_operation(source, move |factor| factor + &change)
    }

    /// Multiplies the current factor. For example, 15_000 means ×1.5.
    pub fn multiplicative(source: impl Into<String>, factor_basis_points: u32) -> Self {
        let multiplier =
            ProductionFactor::new(factor_basis_points.into(), BASIS_POINTS_PER_UNIT.into());
        Self::with_operation(source, move |factor| factor * &multiplier)
    }

    /// Applies a custom operation in strategy registration order.
    pub fn with_operation(
        source: impl Into<String>,
        operation: impl Fn(ProductionFactor) -> ProductionFactor + Send + Sync + 'static,
    ) -> Self {
        Self {
            source: source.into(),
            operation: Arc::new(operation),
        }
    }
}

impl fmt::Debug for ProductionModifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionModifier")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

pub struct ProductionModifierContext<'a> {
    /// The building already has its actual worker allocation for this turn.
    pub building: &'a Building,
    pub biome: Option<Biome>,
}

/// Supplies one yield operation when its conditions are met.
pub trait ProductionModifierStrategy: std::fmt::Debug + Send + Sync {
    fn modifier(&self, context: &ProductionModifierContext<'_>) -> Option<ProductionModifier>;
}

impl ProductionModifierStrategy for ProductionModifier {
    fn modifier(&self, _: &ProductionModifierContext<'_>) -> Option<ProductionModifier> {
        Some(self.clone())
    }
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

    pub(crate) fn apply_modifiers(
        self,
        modifiers: impl IntoIterator<Item = ProductionModifier>,
    ) -> ProductionYield {
        let zero = ProductionFactor::from_integer(0.into());
        if !self.reachable {
            return ProductionYield(zero);
        }

        let initial = ProductionFactor::new(self.numerator.into(), self.denominator.into());
        let factor = modifiers
            .into_iter()
            .fold(initial, |factor, modifier| (modifier.operation)(factor));
        ProductionYield(factor.max(zero))
    }
}

pub(crate) struct ProductionYield(ProductionFactor);

impl ProductionYield {
    pub(crate) fn scale_output(
        &self,
        amount: u32,
        work: u128,
        capacity: u128,
        multiplier: u32,
    ) -> u64 {
        let mut numerator = self.0.numer() * amount * work;
        let mut denominator = self.0.denom() * capacity;
        if multiplier != BASIS_POINTS_PER_UNIT {
            numerator *= multiplier;
            denominator *= BASIS_POINTS_PER_UNIT;
        }
        u64::try_from(numerator / denominator).unwrap_or(u64::MAX)
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct TurnProductionModifiers<'a> {
    pub global: Option<&'a ProductionModifier>,
    pub material_multipliers: Option<&'a HashMap<ResourceType, u32>>,
}

impl TurnProductionModifiers<'_> {
    pub(crate) fn material_multiplier(&self, resource: &ResourceType) -> u32 {
        self.material_multipliers
            .and_then(|multipliers| multipliers.get(resource))
            .copied()
            .unwrap_or(BASIS_POINTS_PER_UNIT)
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
            ratio.apply_modifiers([]).scale_output(
                u32::MAX,
                u128::from(u32::MAX),
                u128::from(u32::MAX),
                BASIS_POINTS_PER_UNIT,
            ),
            u64::from(u32::MAX - 1)
        );
        assert_eq!(policy.productivity(Some(u32::MAX)).as_factor(), 0.0);
    }

    #[test]
    fn work_and_location_are_combined_before_rounding_output() {
        let ratio = TravelCostEfficiencyPolicy::new(0, 30)
            .unwrap()
            .productivity(Some(10));
        assert_eq!(
            ratio
                .apply_modifiers([])
                .scale_output(3, 1, 2, BASIS_POINTS_PER_UNIT),
            1
        );
    }
}
