# 0005 - World and Blueprint Encapsulation

Status: implemented. Summary updated October 29, 2026.

## Context

Public world layers allowed callers to change buffer shape, province identities
or membership without coordination. Mutable building definitions required recipe
validation at every consumer, despite definitions being shared as flyweights.

## Decisions

1. **Protect world layers.** Expose map and provinces through shared views.
   Edit existing tiles through checked commands; synchronize province tile lists
   when membership changes. Resolve province centroids by identity, not vector index.
2. **Validate blueprints once.** `BuildingDefinition::new` returns a validated,
   immutable definition or `DefinitionError`. Keep fields private and provide getters.
3. **Trust validated values.** Building creation, upgrades and catalogue registration
   no longer repeat definition validation or return a definition error. Project
   commands still check identities, target existence and spatial reservations.
4. **Preserve lifecycle rules.** Terrain changes are observed on the next turn.
   Province edits retain structural identities; existing preflight rejects spending
   outside a settlement's province. Cancellation remains possible.

## Still to decide

Trade and caravan lifecycle/access rules remain outside this change, pending their
separate work. This decision does not define ownership transfer or automatic
demolition when province membership changes.

## Details

- [Buildings](../architecture/systems/buildings.md)
- [World tile commands](../../core/src/world/state/map_commands.rs)
- [Blueprint validation tests](../../core/tests/economy_validation_tests.rs)
