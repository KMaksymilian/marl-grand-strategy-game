

Status: implemented. Summary updated October 29, 2026.

## Context

Large files made related operations difficult to find. Split methods by responsibility
while preserving their behavior. This record covers method grouping; the subsequent
directory reorganization is described in decision 0003.

## Decisions

1. **One settlement interface.** Keep commands on `Settlement`, using additional
   implementation blocks in private child modules instead of a separate API object.
2. **Group related methods.** Separate inventory, building and project commands.
   Keep turn execution, project lifecycle, building production, trade matching and
   world-generation operations in dedicated files.
3. **Protect internal state.** Keep settlement managers and their collections private.
   Expose validated commands and read-only views.
4. **Keep tests near their subject.** Place unit tests alongside the implementation
   and public integration tests in the crate's `tests/` directory.
5. **Give documentation clear roles.** Use `architecture/` for implemented systems,
   `decisions/` for rationale, `planning/` for proposals and `analysis/` for dated reviews.
6. **Maintain navigation.** Update the documentation index and source map when files
   or methods move. List runtime methods in the source map; keep test names and
   derived methods in source or generated documentation.

## Still to decide

None recorded for this decision.

## Details

- [Project structure](../architecture/implementation/project_structure.md)
- [Source file and method map](../architecture/implementation/code_map.md)
- [Functional modules](0003-functional-modules.md)
- [Documentation index](../README.md)
