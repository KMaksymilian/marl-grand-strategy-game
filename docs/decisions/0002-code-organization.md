# Code Organization

Status: implemented on `fix/code-organization`, based on develop `8335e26`.

## Decision

Organize the existing modules by responsibility while preserving simulation
behavior. `Settlement` remains the single aggregate and public command surface.
Its state and reads live at the module root; commands are grouped under
`commands/{inventory,buildings,projects}.rs`. `turn.rs` coordinates the turn and
`project_lifecycle.rs` validates/applies project effects. These are additional
`impl Settlement` blocks in private child modules. Managers stay private.

Expose the runtime building model from `buildings`, separate validated definition
data from the factory registry, and locate caravan transport within `trade`.
Group production methods under `building/production.rs`, matching methods under
`trade_log/matching.rs`, and terrain/province generation under
`world_builder/{terrain,provinces}.rs`. Keep type state and pipeline orchestration
in their parent modules. Move existing method bodies without changing algorithms;
only private visibility is widened to the owning module where cross-file calls
require it. Public calls and imports remain unchanged by this additional split.
Module path changes are listed in the [structure guide](../architecture/project_structure.md).

Keep unit tests inline with the implementation and public integration tests under
the crate's `tests/` directory. The reorganization adds no dependencies or runtime
configuration fields.

## Documentation

Use `architecture/` for implemented systems, `planning/` for future behavior,
`decisions/` for rationale and `analysis/` for dated reviews. Rename `economic.md`
to `economy.md`, fix `agregates.md` to `planning/aggregates.md`, and move the economy
roadmap to `planning/`. Maintain the [documentation index](../README.md) and
[source map](../architecture/code_map.md) whenever files or methods move.

The source map documents explicitly implemented runtime methods, including private
helpers, and identifies data-only and module-registration files. Inline test names
and compiler-generated trait methods stay in source/rustdoc rather than being
duplicated in the manual index.
