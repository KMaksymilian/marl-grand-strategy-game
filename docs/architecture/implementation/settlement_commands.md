# Settlement Commands

This document describes the implemented settlement model. Accounting rules are documented in [economy rules](economy_rules.md); prototype trade behavior is described in [trading.md](../systems/trading.md). See the [code map](code_map.md) for file responsibilities and method descriptions.

## 1. Scope

`Settlement` coordinates the local population, inventory, buildings and projects. It is intended as a future decision point for a MARL agent, but currently contains no AI model, policy, observations or reward.


## 2. Model Components

| `Settlement` field | Type | Responsibility |
| --- | --- | --- |
| `id` | `SettlementId` | Immutable identity within the world's registry |
| `inventory_manager` | `SettlementInventoryManager` | Warehouse and production buffer |
| `building_manager` | `BuildingManager` | Building collection, productivity calculator, requests, production and upgrades |
| `population_manager` | `PopulationManager` | Population count, needs registry and grant evaluation |
| `project_manager` | `ProjectManager` | Active projects, remaining costs and completion |

The system-level [settlement class view](../systems/settlements.md#local-class-view) and
[aggregate ownership view](../overview/state_ownership.md#aggregate-and-ownership-view) describe these relationships.

`Settlement::new(id: SettlementId, initial_capacity, initial_population, need_registry)` creates an empty warehouse and buffer with the requested capacity, no buildings or projects.

Each settlement owns an independent ordered storage list. `set_storage_order(materials: impl AsRef<[ResourceType]>)` validates it against the material catalogue; `storage_order()` returns a read-only slice. See [economy rules](economy_rules.md).

Settlement fields and manager modules are private implementation details. Public reads use `id()`, `population()`, `inventory()`, `pending_inventory()`, `buildings()` and `active_projects()`. Building and project views expose immutable identity, definitions and remaining costs through accessors. There is no mutable collection getter.

The model, constructor and read-only views live in `settlement/mod.rs`. Mutation commands live in the private `commands/` directory, grouped by inventory, buildings and projects.

The aggregate also owns a `GoldInventory` and `TradeBook`, both initially empty. \

Use `deposit()`, `withdraw()` and `set_inventory_capacity()` to change warehouse stock or capacity. Create definitions through fallible `BuildingDefinition::new`, then instances through infallible `Building::new`. Add these buildings through `add_building()`; duplicate IDs are rejected. Change a managed building through `set_building_load()` or `set_building_active()`, which reject missing IDs. Queue projects through `queue_construction()` and `queue_upgrade()`, which accept already validated definitions and check local project/target identities before any allocation. `NeedRegistry` and `NeedTier` are re-exported for external needs configuration.

## 5. Construction and Upgrade Projects

`Project` stores a `ProjectId`, `ProjectResult` and `remaining_cost: ProjectCost`. Cost consists of `needed_resources: HashMap<ResourceType, u32>` and `turn_cost: u32`.

| Project result | Action on completion |
| --- | --- |
| `ConstructBuilding` | Creates a building at the supplied position with the supplied definition |
| `UpgradeBuilding` | Replaces the definition of the target `BuildingId` |
| `RecruitUnit` | Explicitly rejected with `UnsupportedRecruitment` |

Recruitment is currently unsupported: an active `RecruitUnit` project is rejected with `UnsupportedRecruitment` before it can receive materials. No unit is created.

`queue_construction` and `queue_upgrade` accept a caller-supplied project ID and explicit `ProjectCost`. They do not automatically use `BuildingDefinition.construction_cost`. The definition cost is available as data; consistency with the command cost remains the caller's responsibility.

Zero material entries are removed when a project is created or registered and after allocation. Demand excludes zero entries defensively. `is_fully_paid()` checks that all remaining amounts are zero and time is zero, so a stray zero entry cannot block completion.

Each turn, a valid project requests its entire remaining material cost. `resolve_allocation` subtracts grants with `saturating_sub` and removes entries reduced to zero. The time counter decreases by 1 if positive, even without material delivery. It represents elapsed turns since ordering, rather than work turns after payment. This timing rule is unchanged.

Example: a project costs 10 wood and 2 turns. Turn one delivers 4 wood, leaving 6 wood and 1 turn. Turn two delivers nothing, leaving 6 wood and 0 turns. Turn three delivers 6 wood; the project completes during finalization of that turn.

`ProjectManager` advances all active projects concurrently. There is no single construction slot or queue that processes only the first item. `extract_completed()` removes completed projects while preserving the order of remaining ones.

## 6. ID Uniqueness and Command Results

| Operation | Result |
| --- | --- |
| `construct_building(building, source)` | `Result<(), BuildingId>`: an error identifies an existing building with that ID |
| `add_project(project)` | `Result<(), ProjectId>`: an error identifies an existing active project with that ID |
| `queue_construction(...)`, `queue_upgrade(...)` | `Result<(), ProjectError>`: report duplicate projects, missing upgrade targets or invalid spatial reservations before accepting a command |
| `next_available_id()` | The first free `BuildingId`, starting at 0 and checked against all buildings |

A completed construction receives a building ID independently of its project number. A project ID is not a global identity for the resulting object. Typed arguments prevent passing a `ProjectId` as an upgrade target.

Validation applies to commands. Collections and runtime fields are protected from external mutation. There is no completed-project history; a project number can be reused after it leaves the active list.

## 7. Turn Report and Project Outcomes

The public report is defined in [turn_report.rs](../../../core/src/settlement/turn/report.rs):

```rust
pub struct TurnReport {
    pub consumption: ConsumptionResult,
    pub projects: Vec<ProjectOutcome>,
    pub inventory: InventoryTurnReport,
}

pub struct ProjectOutcome {
    pub project_id: ProjectId,
    pub result: Result<ProjectCompletion, ProjectError>,
}
```

Successful outcomes identify `BuildingConstructed(BuildingId)` or `BuildingUpgraded(BuildingId)`. Errors include `MissingBuilding`, `UnsupportedRecruitment` and duplicate IDs. Construction allocates its own building ID; upgrades preserve the target ID and position. Population evaluation is now accessed through `report.consumption`.

Queue errors are returned immediately and do not enter the active list. At turn start, invalid active projects are removed and reported before material withdrawal. Finalization also checks the target and handles the effect result explicitly. Pending projects produce no outcome entry. An outcome is emitted only in the turn that removes its project; no history is stored in the settlement. Pre-allocation rejections appear first, followed by finalization outcomes, preserving project order within each phase.

If an upgrade target disappears after earlier partial payment, the next turn reports failure without further allocation. Previously paid materials remain spent. Explicit cancellation follows the same no-refund rule.

`inventory` records local material stock, production, internal returns, consumption categories and stage-specific overflow losses. `inventory.is_balanced()` checks this accounting separately for each material, excluding renewed labour. Trade reservations/returns are not accounted for, so this check does not establish conservation for nonempty trade activity. The local balance equation is documented in [economy rules](economy_rules.md); trade limitations are in [trading.md](../systems/trading.md).

## Spatial Registration and Construction

`World` owns the global `TerritoryManager` and registers each settlement in exactly one province with `add_settlement(province_id, settlement)`. A province may have no settlement. Register settlements before adding buildings or projects; importing existing structures is not supported yet.

For registered settlements, use `World::queue_construction`, `World::add_building`, `World::queue_upgrade`, `World::cancel_project` and `World::process_settlement_turn`. Construction entry points validate the settlement and its province, then delegate lifecycle changes to `Settlement`; cancellation only requires the settlement and its matching reservation. The world's registry and settlements have no public mutable getters. `World::inspect_tile(position)` combines terrain, province, optional settlement and occupancy with borrowed building/project data; see [territory](../systems/territory.md).

Standalone construction/cancellation receives `&mut impl ConstructionSpace`; `add_building` and `process_turn` require both `ConstructionSpace` and `TravelCostSource`. The caller must retain and reuse the same backing occupancy store. The supplied world adapters preserve land and overlap validation. Project and building references include the settlement ID because their local IDs can overlap across settlements.

New construction requires a valid land tile and an empty structural slot. Land uses the existing `Elevation::OCEAN_LEVEL` boundary; building-specific marine placement rules are not implemented. Queueing reserves `Construction(ProjectReference)` without spending materials. Successful completion replaces it with `Building(BuildingReference)`. Direct building registration performs the same placement and occupancy checks. Upgrades keep the existing building on its tile throughout the project.

Construction sites are checked again before allocation and completion. Invalid construction is reported and releases only its own reservation; previously paid materials are not refunded. If mutable province layers change membership, the world entry point rejects the turn before spending, leaving projects and reservations unchanged.

`Settlement::cancel_project(project_id, territory)` removes an active project and stops future spending. For new construction it first releases the exact `(settlement_id, project_id)` reservation. If that reservation differs, cancellation fails without removing the project or changing the tile. Canceling an upgrade preserves the existing building and its definition. Already paid materials remain consumed; unpaid costs are discarded. Cancellation is immediate and does not emit a later turn outcome. Missing or completed projects return `MissingProject`.

`World::cancel_project(settlement_id, project_id)` delegates to the settlement with the shared registry. Cancellation does not require current province membership, allowing a project to be stopped after map edits.

## 8. Limitations and Open Questions

- Recruitment remains unimplemented and is explicitly rejected.
- There is no cost refund or automatic population change. `Simulation::step` runs global phases; `World::process_settlement_turn` remains a local convenience entry point. See [simulation commands](simulation_commands.md).
- Overflow uses a configurable ordered material list. There is no automatic policy choosing this order. Resource maps are private; capacity changes use validated commands.

These limitations describe current code rather than approving final game rules. Policies requiring further decisions are collected in [0001-economy.md](../../decisions/0001-economy.md).

## 9. Sources

Model and views: [settlement/mod.rs](../../../core/src/settlement/mod.rs). Commands: [inventory](../../../core/src/settlement/commands/inventory.rs), [buildings](../../../core/src/settlement/commands/buildings.rs), [projects](../../../core/src/settlement/commands/projects.rs). Turn processing and inline unit tests: [turn.rs](../../../core/src/settlement/turn/mod.rs). Project effects: [project_lifecycle.rs](../../../core/src/settlement/projects/lifecycle.rs). Managers: [building_manager.rs](../../../core/src/settlement/economy/buildings.rs), [project_manager.rs](../../../core/src/settlement/projects/manager.rs), [inventory_manager.rs](../../../core/src/settlement/economy/inventory.rs), [population.rs](../../../core/src/settlement/economy/population.rs). Projects: [projects.rs](../../../core/src/settlement/projects/mod.rs).
