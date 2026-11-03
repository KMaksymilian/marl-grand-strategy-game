# 0003 - Functional Modules and Spatial Dependencies

Status: implemented. Summary updated October 29, 2026.

## Context

The `domain/services` split scattered related code across technical directories.
Caravan construction also depended on the whole world and confused participant
identities with province indices. This decision follows the method grouping in 0002.

## Decisions

1. **Group by function.** Place related models and operations together, such as
   economy, settlements and trade. Keep terrain, generation and pathfinding under `world`.
2. **Use an unambiguous package name.** Name the Rust package `simulation_core`
   while retaining the `core/` directory, avoiding confusion with Rust's standard library.
3. **Keep coordinate contracts explicit.** Store `Point` and `Position` together,
   preserving unsigned grid indices and signed placement coordinates.
4. **Pass spatial dependencies directly.** Construct caravans using a map, pathfinder,
   participant IDs, explicit endpoints and a trade book. Return `None` for invalid
   or unreachable routes instead of requiring the whole `World`.
5. **Make migration explicit.** Remove old namespaces without compatibility aliases.
   Update imports and the caravan constructor contract; preserve economic arithmetic
   and movement behavior as part of this reorganization.

## Still to decide

None recorded for this decision.

## Details

- [Code organization](0002-code-organization.md)
- [Project structure and import migration](../architecture/implementation/project_structure.md)
- [Source file and method map](../architecture/implementation/code_map.md)
- [Trading and caravans](../architecture/systems/trading.md)
