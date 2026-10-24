# Building Productivity and A* Travel Costs

## Model and ownership

Buildings use one collection. Hub/Spoke and `BuildingLocation` are removed from instances, construction commands and project results. `Arc<BuildingDefinition>` remains a Flyweight, and `BuildingBehavior` composes recipe/stat data without a class hierarchy. Stat modifiers are still not executed.

`BuildingManager` owns the optional `BuildingProductivityCalculator`, which holds the capital position and a validated `TravelCostEfficiencyPolicy`. The manager calculates and updates productivity on insertion, configuration changes and every turn. `Settlement` delegates these operations to the manager. A building stores only the computed exact multiplier, with no reference to the map or settlement and no extra lifetime parameter.

## Configuration and the fraction

Balance settings live in the repository-root [configuration.toml](../../configuration.toml), under:

```toml
[economy.building_productivity]
full_productivity_cost = 100
zero_productivity_cost = 300
```

These initial values can be edited without changing Rust code. There are no hidden fallback thresholds. `Configuration::load(path)` reads the file and validates required fields, integer ranges and `full_productivity_cost < zero_productivity_cost`. Unknown fields and malformed TOML return errors. The application reads this file from its working directory at startup; it does not hot-reload automatically.

For a reachable building, let `cost` be the directed minimum A* movement cost from the capital to its position. The rule is:

```text
productivity = 1, if cost <= full_productivity_cost
productivity = 0, if cost >= zero_productivity_cost

Between the thresholds:
numerator   = zero_productivity_cost - cost
denominator = zero_productivity_cost - full_productivity_cost
productivity = numerator / denominator
```

The numerator varies with the actual route; both of its balance parameters and the denominator's balance parameters come from TOML. With the supplied settings, A* cost 100 gives 100%, cost 200 gives 50%, and cost 300 gives 0%. Unreachable positions give 0%, including water endpoints, negative or out-of-map positions. This is an output penalty, not a building placement restriction.

## The same A* as the map view

`AStar::travel_cost()` runs the same search used by `AStar::a_star()`, without constructing a path when only the cost is needed. Both use `PathFinder::neighbors8`, `is_ocean` and `move_cost`. There is no Euclidean distance in the productivity calculation and no separate terrain-cost formula.

The existing movement rules have straight cost 10, diagonal cost 14, blocked ocean tiles and slope penalties, with a larger penalty uphill. Accordingly, a ten-tile flat straight route costs 100; an obstacle detour or difficult ascent costs more. Uphill and downhill costs can differ: the policy uses the route from capital to building, not a round trip. Existing diagonal corner traversal rules are unchanged. Roads are not yet implemented.

Before every economic turn, `Settlement::process_turn(map, finder)` asks `BuildingManager` to recompute each building's route cost from the current map. Terrain edits therefore affect the next turn, even when the capital and building positions stay unchanged. `set_building_productivity_calculator()` and `add_building()` also receive the map and finder to calculate an immediately correct cached multiplier. Construction completion uses the same path. An upgrade retains the position and takes the current map into account at the next turn.

For B buildings this version performs B A* queries per refresh, reusing the caller's pathfinding buffers. It does not cache routes between turns, so it cannot silently keep a cost from an older map. `PathFinder` generation wrap clears old stamps. A later optimization could batch queries using a shared shortest-cost field with exactly the same movement rules.

## Operating load and material accounting

| Concept | Meaning |
| --- | --- |
| `target_load: f32` | Requested fraction of staffing and inputs, validated in 0..=1 |
| Recipe work fraction | Actual execution limited by granted workers and remaining inputs |
| `location_productivity()` | Cached output multiplier computed from the current A* travel cost |

`target_efficiency` is renamed to `target_load`. A new building starts active at load zero. Without a configured calculator (`None`), location productivity is 1. The settlement constructor retains its existing signature; configuring a capital explicitly enables the policy.

```text
work = min(granted_workers / max_workers, remaining_input / base_input, ...)
consumed_input = ceil(base_input * work)
produced_output = floor(base_output * work * location_productivity)
```

Travel difficulty reduces output only. A fully supplied workshop needing 10 workers and 10 wood to produce 20 stone still uses that work and wood at 50% productivity but produces 10 stone. At zero productivity it can still incur operating costs. Set its load to zero or deactivate it to stop operating it.

Multipliers are stored as exact fractions. Output combines work and location fractions with `u128` arithmetic before one rounding. Resource quantities stay in whole `u32` units. Public factor accessors use `f64` only for inspection. Material reports record the reduced actual production; output never produced is not an overflow loss.

## Usage

Given an existing `map`, reusable `finder` and settlement:

```rust
use simulation_core::configuration::Configuration;
use simulation_core::world::Position;
use simulation_core::buildings::productivity::BuildingProductivityCalculator;

let configuration = Configuration::load("configuration.toml").unwrap();
let calculator = BuildingProductivityCalculator::new(
    Position { x: 100, y: 50 },
    configuration.economy.building_productivity,
);
settlement.set_building_productivity_calculator(Some(calculator), &map, &mut finder);
let report = settlement.process_turn(&map, &mut finder);
```

Reload the configuration and set a new calculator to apply changed thresholds to existing settlements. Passing `None` restores full location productivity. Each settlement has its own capital; callers normally supply the same configured policy to all settlements in an experiment. The application currently renders the world and validates configuration, but does not yet create settlements or run a global economy loop.

## Migration and tests

- Remove `BuildingLocation` from `Building::new`, `queue_construction` and project results.
- Use `target_load`, `set_target_load`, `set_building_load`, `LoadError` and `InvalidLoad` instead of the previous efficiency names.
- Pass `&Map` and `&mut PathFinder` to `process_turn`, `add_building` and `set_building_productivity_calculator`.
- Replace the earlier Euclidean `DistanceEfficiencyPolicy` with `TravelCostEfficiencyPolicy`, configured in movement-cost units.

Unit tests live with their implementations. Integration tests in `core/tests/building_productivity_tests.rs` cover configuration-driven production, obstacles, changing terrain, elevation, unreachable buildings, construction/upgrades, shortages, rounding and material conservation.
