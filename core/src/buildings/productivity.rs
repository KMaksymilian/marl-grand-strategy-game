use crate::world::coordinates::Position;
use crate::world::map::Map;
use crate::world::pathfinding::{a_star::AStar, path_finder::PathFinder};

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
            return LocationProductivity::ZERO;
        };
        if cost <= self.full_productivity_cost {
            LocationProductivity::FULL
        } else if cost >= self.zero_productivity_cost {
            LocationProductivity::ZERO
        } else {
            LocationProductivity {
                numerator: self.zero_productivity_cost - cost,
                denominator: self.zero_productivity_cost - self.full_productivity_cost,
            }
        }
    }
}

/// Computes building output multipliers from capital-to-building A* route costs.
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

    /// Directed route cost from the capital to the building on the supplied map.
    pub fn travel_cost_to(
        self,
        position: Position,
        map: &Map,
        finder: &mut PathFinder,
    ) -> Option<u32> {
        let start = (
            usize::try_from(self.capital.x).ok()?,
            usize::try_from(self.capital.y).ok()?,
        );
        let end = (
            usize::try_from(position.x).ok()?,
            usize::try_from(position.y).ok()?,
        );
        AStar::travel_cost(map, finder, start, end)
    }

    pub fn productivity_at(self, position: Position, map: &Map, finder: &mut PathFinder) -> f64 {
        self.multiplier_at(position, map, finder).as_factor()
    }

    pub(crate) fn multiplier_at(
        self,
        position: Position,
        map: &Map,
        finder: &mut PathFinder,
    ) -> LocationProductivity {
        self.policy
            .productivity(self.travel_cost_to(position, map, finder))
    }
}

/// Exact internal ratio, independent of resource units. Only valid policies create it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LocationProductivity {
    numerator: u32,
    denominator: u32,
}

impl LocationProductivity {
    pub(crate) const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    pub(crate) const FULL: Self = Self {
        numerator: 1,
        denominator: 1,
    };

    pub(crate) fn as_factor(self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }

    pub(crate) fn scale_output(self, amount: u32, work: u128, capacity: u128) -> u32 {
        // Each factor is bounded by a validated u32 recipe total or travel-cost threshold.
        u32::try_from(
            u128::from(amount) * work * u128::from(self.numerator)
                / (capacity * u128::from(self.denominator)),
        )
        .expect("productivity cannot increase recipe output")
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
            ratio.scale_output(u32::MAX, u128::from(u32::MAX), u128::from(u32::MAX)),
            u32::MAX - 1
        );
        assert_eq!(policy.productivity(Some(u32::MAX)).as_factor(), 0.0);
    }

    #[test]
    fn work_and_location_are_combined_before_rounding_output() {
        let ratio = TravelCostEfficiencyPolicy::new(0, 30)
            .unwrap()
            .productivity(Some(10));
        assert_eq!(ratio.scale_output(3, 1, 2), 1);
    }
}
