# Functional Modules and Explicit Spatial Dependencies

Status: implemented on `fix/code-organization`, following the initial method
grouping in [0002](0002-code-organization.md).

## Decision

Replace the broad `domain/services` split with functional modules at the library
root: `world`, `economy`, `buildings`, `settlement`, `trade`, `ids` and
`configuration`. World state, terrain classification, generation and pathfinding
belong together under `world`. The Rust package becomes `simulation_core` to
avoid confusion with Rust's standard `core`; its workspace directory stays `core/`.

Separate resource catalogue data (`economy/resources.rs`) from stock management
(`economy/inventory.rs`), re-exporting the common types at `economy`. Rename the
blueprint registry to `BuildingCatalog`, active funded offers to `TradeBook`, and
spatial seed/assignment/centroid helpers to `Voronoi`. Shorten trade file names
because their parent module already supplies the trade context.

Keep `Point` and `Position` together in `world/coordinates.rs`, preserving their
unsigned and signed representations respectively. Their numeric contracts are
not changed by this reorganization.

## Caravan dependency

Previously `World` owned caravans while `Caravan::new()` required a whole `World`
and treated participant IDs as province indices. The constructor now receives
`&Map`, `&mut PathFinder`, participant IDs, explicit start/end `Point` values and
a `TradeBook`. Its return type is `Option<Caravan>` so invalid or unreachable
routes can be handled by the caller. `World` still owns and advances caravans.

This is an intentional constructor-contract change, with inline tests for explicit
endpoints/opaque participant IDs and absent routes. Matching, travel speed,
return movement, local turn order and economic arithmetic remain unchanged.

## Consequences

Public imports change; no aliases preserve the removed technical namespaces.
No new dependency, trait hierarchy, API wrapper, crate split or simulation loop is
introduced. `Cargo.lock` changes only to rename the local package and references
to it. See the [structure guide](../architecture/project_structure.md) for the
complete migration table and [code map](../architecture/code_map.md) for sources.
