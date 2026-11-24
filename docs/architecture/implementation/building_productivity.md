# Building Productivity and A* Travel Costs

## Model and ownership


`BuildingManager` owns the optional `BuildingProductivityCalculator`, which holds the capital position and a validated `TravelCostEfficiencyPolicy`. The manager calculates and updates productivity on insertion, configuration changes and every turn.

## Configuration and the fraction

Balance settings live in the repository-root [configuration.toml](../../../configuration.toml), under:

```toml
[economy.building_productivity]
full_productivity_cost = 100
zero_productivity_cost = 300
```

These initial values can be edited without changing Rust code.

For a reachable building, let `cost` be the directed minimum A* movement cost from the capital to its position. The rule is:

```text
productivity = 1, if cost <= full_productivity_cost
productivity = 0, if cost >= zero_productivity_cost

Between the thresholds:
numerator   = zero_productivity_cost - cost
denominator = zero_productivity_cost - full_productivity_cost
productivity = numerator / denominator
```



## Operating load and material accounting

| Concept | Meaning |
| --- | --- |
| `target_load: f32` | Requested fraction of staffing and inputs, validated in 0..=1 |
| Recipe work fraction | Actual execution limited by granted workers and remaining inputs |
| `location_productivity()` | Cached yield factor from the current A* travel cost |

`target_efficiency` is renamed to `target_load`. A new building starts active at load zero. Without a configured calculator (`None`), location productivity is 1. When a policy is configured, the registered settlement capital supplies its A* origin. Registration and later policy changes keep the calculator aligned with that capital.

```text
work = min(granted_workers / max_workers, remaining_input / base_input, ...)
consumed_input = ceil(base_input * work)
yield_factor = max(0, apply_modifiers_in_registration_order(location_productivity))
produced_output = floor(base_output * work * yield_factor)
```

An unreachable A* route forces zero output even with positive modifiers. A reachable
route at the zero-productivity threshold contributes a penalty of 100%.

Register sources with `Settlement::add_production_modifier_strategy`. Each strategy
can inspect the building, its allocated workers and the current tile biome.
A modifier that always applies can be registered directly with the same method.
`BiomeProductionModifier` supplies an optional rule for a building type and biome.
Use `ProductionModifier::new("decree", 2_000)` to add 20 percentage points
or `ProductionModifier::multiplicative("tools", 15_000)` to multiply by 1.5.
Use `ProductionModifier::with_operation` to supply a new operation on the exact
factor, such as capping its value. The same calculator applies all operations in
the order their strategies were registered. For a starting factor of 1,
adding 0.2 then multiplying by 1.5 yields 1.8. Reversing them yields 1.7.
The rules apply after allocation and leave recipe input consumption unchanged.
Active production decrees match each output's resource category and tier. Their
matching changes add together for that output, then multiply its yield before
the final rounding. This applies to all recipe output paths.
Register terrain rules when configuring a settlement. For a temporary effect
such as a decree, the strategy should return a modifier only while it applies.
