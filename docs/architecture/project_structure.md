# Project Structure

This guide describes the implemented repository. Proposed systems belong in
[planning](../planning/aggregates.md). Use the [code map](code_map.md) to locate
individual types and methods.

## Workspace

```text
.
├── Cargo.toml                       Rust workspace membership
├── Cargo.lock                       Resolved dependencies
├── configuration.toml               Runtime economy configuration
├── core/
│   ├── src/
│   │   ├── configuration.rs         TOML parsing and validation
│   │   ├── domain/
│   │   │   ├── buildings/           Instances, definitions, registry, productivity
│   │   │   ├── economy/             Stock, currency, allocation, storage order
│   │   │   ├── geography/           Terrain classification
│   │   │   ├── settlement/          Settlement state, commands, turns and managers
│   │   │   ├── trade/               Offers, matching, reservations and caravans
│   │   │   ├── world_data/          Map, provinces, coordinates and world state
│   │   │   └── ids.rs               Typed building and project identifiers
│   │   └── services/
│   │       ├── algorithms/          A*, Dijkstra and reusable search buffers
│   │       └── world_generation/    Terrain generation, noise and Voronoi
│   └── tests/                       Tests through the public domain interface
├── adapters/src/view/map/           Macroquad input, textures and rendering
├── main_app/src/main.rs             Configuration loading and application loop
├── ml_brain/src/requirements.txt    Empty Python dependency placeholder
├── docs/                           Documentation; see docs/README.md
└── .github/workflows/               Rust CI definition
```

`main_app` composes `core` and `adapters`. `adapters` reads the domain to render
it; `core` has no dependency on Macroquad. Configuration parsing lives at the
core crate boundary. Spatial calculations live in services and are used by
domain productivity and caravan routing.

The executable displays a generated world and interactive pathfinding overlays.
Settlement turns and trading primitives exist in the library; they are not wired
into a global simulation loop. AI training and communication ports are future work.

## File responsibilities

- `mod.rs` declares a module and exposes its intended public surface. The
  settlement root also owns its aggregate state, constructor and read-only views.
- `settlement/commands/` groups inventory, building and project commands in three
  files. They implement methods directly on `Settlement`. `settlement/turn.rs`
  contains the turn sequence; `project_lifecycle.rs` contains validation and
  completion effects. There is no separate API wrapper or borrowed API object.
- Managers remain private children of `settlement`, with access limited to the
  aggregate's implementation. Commands preserve their existing return types.
- `buildings/building.rs` owns runtime building state. `definition.rs` owns shared
  blueprints, behavior data and validation. `factory.rs` owns their registry.
  `building/production.rs` holds demand, allocation and recipe execution methods.
- `trade/caravan.rs` owns trade transport. `World` retains the collection of
  caravans, while `world_data` contains spatial and world state.
  `trade_log.rs` owns offer/funding state; `trade_log/matching.rs` owns the matching
  algorithm and transaction methods.
- `configuration.rs` parses runtime TOML. `world_generation/config.rs` contains
  generation parameter types and Rust defaults; it is a different concern.
  `world_builder.rs` keeps state, initialization and pipeline orchestration;
  `world_builder/terrain.rs` generates terrain and `world_builder/provinces.rs`
  handles seeds, Voronoi relaxation and coast alignment.

## Finding an operation

```text
domain/settlement/
├── mod.rs                     Aggregate state, constructor and read-only views
├── commands/
│   ├── mod.rs                 Command module declarations
│   ├── inventory.rs           Deposit, withdrawal, capacity and storage order
│   ├── buildings.rs           Registration, load, activity and productivity policy
│   └── projects.rs            Queue construction and upgrades
├── turn.rs                    process_turn and inline aggregate tests
├── project_lifecycle.rs       Validate, reject and finalize projects
└── *_manager.rs               Internal collection/allocation operations

domain/buildings/
├── building.rs                Instance state, lifecycle, settings and views
├── building/production.rs     Demand, grants and recipe execution
├── definition.rs              Blueprint data and validation
├── factory.rs                 Shared definition registry
└── productivity.rs            Travel-cost policy and exact output multiplier

domain/trade/
├── trade_log.rs               Offers, funding and resource collection
├── trade_log/matching.rs      Matching, budget distribution and transactions
├── trade_offer.rs             Buy/sell offer state and demand
├── trade_allocation_engine.rs Reserve and return inventory resources
└── caravan.rs                 Trade transport state and movement

services/world_generation/
├── world_builder.rs          Builder state, initialization and pipeline
├── world_builder/terrain.rs  Elevation, moisture and ocean normalization
├── world_builder/provinces.rs Seeds, Voronoi relaxation and coast alignment
├── world_builder_functions.rs Reusable geometry/seed helpers
├── noise_manager.rs          Noise sampling and coordinate warp
└── config.rs                 Generation parameter types and defaults
```

Use the owning type to call a method regardless of the implementation file:
`settlement.deposit(...)`, `building.resolve_allocation(...)`,
`trade_log.trade(...)`, `WorldBuilder::generate(...)`. Private child modules group
the implementation without adding objects or changing public import paths.

## Imports after the organization change

| Previous path | Current path |
| --- | --- |
| `core::domain::settlement::core::Settlement` | `core::domain::settlement::Settlement` |
| `core::domain::settlement::core::BuildingCommandError` | `core::domain::settlement::BuildingCommandError` |
| `core::domain::buildings::core::{Building, Position, ...}` | `core::domain::buildings::{Building, Position, ...}` |
| `core::domain::buildings::factory::{BuildingDefinition, BuildingBehavior, DefinitionError}` | `core::domain::buildings::definition::{BuildingDefinition, BuildingBehavior, DefinitionError}` |
| `core::domain::world_data::caravan::{Caravan, CaravanState}` | `core::domain::trade::caravan::{Caravan, CaravanState}` |

`BuildingFactory` remains in `buildings::factory`. Public method signatures and
simulation rules are unchanged. External callers must migrate the listed module
paths; the old `core` modules are removed. Private file renames are
`demographics.rs` → `population.rs` and
`settlement_inventory_manager.rs` → `inventory_manager.rs`.

## Tests and documentation maintenance

Keep unit tests inline in the file implementing their feature, under
`#[cfg(test)] mod tests`. The turn tests remain inside `turn.rs`, including tests
that inspect private aggregate state. Cross-feature/public-interface tests belong
in `core/tests/`. Compile-fail examples protect encapsulation in rustdoc.

When adding or moving a source file, update its entry in [code_map.md](code_map.md),
including methods and a short responsibility description. Record current behavior
in `architecture/`, proposals in `planning/`, and decisions in `decisions/`.
Historical analyses retain their dated paths and conclusions.

Run from the repository root:

```sh
cargo run -p marl_grand_strategy
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps
```

The executable package is named `marl_grand_strategy`.
