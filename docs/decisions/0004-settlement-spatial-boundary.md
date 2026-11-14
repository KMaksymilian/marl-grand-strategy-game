# 0004 - Map-independent Settlement

Status: implemented. Summary updated October 29, 2026.

## Context

Local construction and production depended directly on map terrain, pathfinding
buffers and the global occupancy registry. Building definitions also imported
shared values from the settlement, creating dependencies in both directions.

## Decisions

1. **Depend on capabilities.** Use `ConstructionSpace` for reservations and
   `TravelCostSource` for directed movement costs. Keep map rules and A* in world adapters.
2. **Keep state owned.** Store no borrowed environment in `Settlement` or `Building`.
   Pass short-lived adapters to operations that need them.
3. **Share neutral values.** Put coordinates/occupancy contracts in `spatial`,
   project costs in `economy` and stat targets in `buildings`. Re-export previous
   public cost/coordinate type paths to ease migration; local command signatures change explicitly.
   The later directory cleanup removes the obsolete `settlement::stat` forwarding
   module; stat targets are imported from `buildings::effects`.
4. **Dispatch once.** Store `SettlementCommand` beside the domain and dispatch
   through `World`; keep simulation decisions as aliases and scheduling outcomes.
5. **Preserve synchronous consistency.** Validate land, membership and exact
   reservations before state changes. Keep economic arithmetic and phase order.

## Still to decide

The independent caravan/world pathfinding dependency remains outside this change.
No routing cache, event bus or automatic economic policy is introduced.

## Details

- [Dependency contracts and diagrams](../architecture/overview/settlement_boundaries.md)
- [Source file and method map](../architecture/implementation/code_map.md)
- [Map-free domain tests](../../core/src/test_support/settlement_environment_tests.rs)
