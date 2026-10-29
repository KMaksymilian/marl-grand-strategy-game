# Project Structure

The simulation is organized by functional area. `core/` contains the Rust package
**`simulation_core`**; the directory keeps its workspace location while imports
use the unambiguous package name. See the [code map](code_map.md) for every source
file and explicitly implemented runtime method.

## Workspace and dependencies

```text
.
├── Cargo.toml                       Workspace: core, adapters, main_app
├── Cargo.lock                       Resolved dependencies
├── configuration.toml               Runtime economy thresholds
├── core/                            Package: simulation_core
│   ├── src/
│   │   ├── lib.rs                   Public functional modules
│   │   ├── configuration.rs         Runtime TOML parsing and validation
│   │   ├── simulation/              Global turns, decisions and reports
│   │   ├── world/                   Spatial data and operations
│   │   │   ├── mod.rs               Public world exports
│   │   │   ├── state.rs             World, settlement registration and province membership
│   │   │   ├── state/construction.rs World entry points for settlement lifecycle
│   │   │   ├── state/inspection.rs  Combined read-only tile view
│   │   │   ├── territory.rs         Global sparse occupancy registry
│   │   │   ├── map.rs               Terrain/territory buffers
│   │   │   ├── province.rs          Province tiles and centroid
│   │   │   ├── coordinates.rs       Point indices and signed Position
│   │   │   ├── terrain/             Area, elevation, moisture and biome
│   │   │   ├── generation/          WorldBuilder, noise and Voronoi
│   │   │   └── pathfinding/         A*, Dijkstra and search buffers
│   │   ├── economy/                 Resources, inventory, currency and allocation
│   │   ├── buildings/               Instances, definitions, catalogue, productivity
│   │   ├── settlement/              Aggregate state, commands, managers and turns
│   │   ├── trade/                   Offers, trade books, matching and caravans
│   │   └── ids.rs                   Typed settlement, building and project identifiers
│   ├── examples/headless.rs         Simulation runner without graphics
│   └── tests/                       Public-interface integration tests
├── adapters/src/view/map/           Macroquad rendering and input
├── main_app/src/main.rs             Application composition and frame loop
├── ml_brain/src/requirements.txt    Python dependency placeholder
├── docs/                           Architecture, decisions, plans and history
└── .github/workflows/               Rust CI definition
```

`main_app` composes `simulation_core` and `adapters`. `adapters` reads simulation
state for rendering; `simulation_core` does not depend on Macroquad. The executable
currently displays a generated map and interactive pathfinding overlays. Settlement
turns can run headlessly through `Simulation`; the graphical loop does not yet
drive it. Trade matching and caravan travel remain outside the coordinator.

`domain/`, `services/`, `world_data/` and the separate top-level `geography/`
directories are removed. World-related models, generation and navigation now live
together, so finding pathfinding or terrain code does not require switching between
technical layers.

## Finding an operation

```text
simulation/
├── mod.rs                     State, validation, registry access and decision queue
├── decisions.rs               Typed commands, dispatch and rejection outcomes
├── turn.rs                    Global phase execution and turn progression
└── turn_report.rs             Completed turn, settlement reports and execution trace

settlement/
├── mod.rs                     State, constructor and read-only views
├── commands/
│   ├── mod.rs                 Private command groups
│   ├── inventory.rs           Deposit, withdrawal, capacity and storage order
│   ├── buildings.rs           Registration, load, activity and productivity
│   └── projects.rs            Queue construction/upgrades and cancel projects
├── turn.rs                    Shared local/global phases and inline aggregate tests
├── project_lifecycle.rs       Validate, reject and finalize projects
├── registry.rs                Ordered persistent settlement store and ID allocation
└── *_manager.rs               Internal collections and allocation

buildings/
├── building.rs                Runtime instance, settings and views
├── building/production.rs     Demand, grants and recipe execution
├── definition.rs              Blueprint data and validation
├── catalog.rs                 BuildingCatalog: shared blueprint registry
└── productivity.rs            Travel-cost policy and exact multipliers

economy/
├── resources.rs               ResourceType and material catalogue
├── inventory.rs               ResourceInventory and admission/capacity results
├── allocation.rs              Proportional local demand allocation
├── storage_order.rs           Ordered material admission policy
└── gold_inventory.rs          Currency counter

trade/
├── book.rs                    TradeBook: offers, funds and resource collection
├── book/matching.rs           Bilateral matching and transactions
├── offer.rs                   Buy/sell offer state and demand
├── allocation.rs              Reserve and return resources
└── caravan.rs                 Route and transport state

world/generation/
├── world_builder.rs           Builder state, initialization and pipeline
├── world_builder/terrain.rs   Elevation, moisture and ocean normalization
├── world_builder/provinces.rs Seeds, Voronoi passes and coast alignment
├── voronoi.rs                 Voronoi: seed/assignment/centroid helpers
├── noise_manager.rs           Noise sampling and coordinate warp
└── config.rs                  Generation parameters and Rust defaults
```

Private child modules add methods to their owning type; callers still use
`settlement.deposit(...)`, `building.resolve_allocation(...)`,
`trade_book.trade(...)` and `WorldBuilder::generate(...)`. `Settlement` is the
public settlement interface. There is no borrowed API wrapper or extra object
needed to call these operations. Managers remain private implementation details.

## Shared concepts and dependency boundaries

`world::coordinates` owns both coordinate types. `Point(usize, usize)` is used for
nonnegative grid indices; `Position { x: i32, y: i32 }` retains the signed placement
contract. They are not merged and no implicit conversion is introduced.

`World` (implemented in `world/state.rs`) owns caravans, but `Caravan` no longer depends on `World`.
Construction accepts a map, search buffer, explicit start/end points, participant
IDs and a `TradeBook`. Callers resolve coordinates; the caravan does not look up
province indices. Missing routes produce `None` rather than a pathfinding panic.

`World` also owns settlements and one global occupancy registry. Each registered
settlement belongs to exactly one province; provinces may be empty. World entry
points validate membership and pass spatial state to `Settlement`, which owns the
project lifecycle. See [territory](territory.md) for placement, cancellation and
`World::inspect_tile`. The executable does not yet expose a tile inspection panel.

`configuration.rs` parses runtime economy TOML. `world/generation/config.rs`
contains generation parameters and defaults; it remains a separate concern.

## Import and name migration

| Previous | Current |
| --- | --- |
| Package `core` | Package/import name `simulation_core`, still at `core/` |
| `core::domain::settlement::Settlement` | `simulation_core::settlement::Settlement` |
| `core::domain::buildings::Building` | `simulation_core::buildings::Building` |
| `core::domain::buildings::Position` | `simulation_core::world::Position` |
| `core::domain::world_data::point::Point` | `simulation_core::world::Point` |
| `core::domain::world_data::world::World` | `simulation_core::world::World` |
| `core::domain::geography` | `simulation_core::world::terrain` |
| `core::services::algorithms` | `simulation_core::world::pathfinding` |
| `core::services::world_generation` | `simulation_core::world::generation` |
| `economy::resource::{ResourceType, ResourceInventory}` | `simulation_core::economy::{ResourceType, ResourceInventory}` |
| `buildings::factory::BuildingFactory` | `simulation_core::buildings::catalog::BuildingCatalog` |
| `trade::trade_log::TradeLog` | `simulation_core::trade::book::TradeBook` |
| `trade::trade_offer` | `simulation_core::trade::offer` |
| `trade::trade_allocation_engine` | `simulation_core::trade::allocation` |
| `world_builder_functions::WorldBuilderFunctions` | `simulation_core::world::generation::voronoi::Voronoi` |

The old technical namespaces are removed, without compatibility aliases. External
callers must migrate imports. Matching rules, production arithmetic, allocation,
turn timing and productivity formulas are unchanged. Caravan construction is the
explicit constructor-contract change described above.

## Tests and documentation

Keep unit tests inline under `#[cfg(test)] mod tests` alongside the feature being
verified. Public integration tests live in `core/tests/`; compile-fail rustdoc
examples protect encapsulation. Tests of caravan construction are inline in
`trade/caravan.rs`.

Use `architecture/` for implemented behavior, `planning/` for proposals,
`decisions/` for rationale and `analysis/` for dated reviews. Update the
[source map](code_map.md) and [documentation index](../README.md) when files or
methods move. Historical analyses retain their original paths and findings.

Run from the repository root:

```sh
cargo run -p marl_grand_strategy
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps
```
