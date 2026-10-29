# Global Simulation Coordinator

`simulation_core::simulation::Simulation` owns a `World`, the number of completed
global turns, reusable pathfinding buffers and queued settlement decisions.
Settlements remain registered only in `World`; the coordinator has no duplicate
settlement collection. `Simulation::settlements()` delegates read access to it.

The implementation is independent of Macroquad and does not initialize a window.
The graphical executable still renders a generated world; it does not yet drive
the coordinator from its frame loop.

## Ownership and responsibilities

The diagram focuses on the coordinator and its direct dependencies. Composition
means ownership of Rust values; `SettlementDecision` is an enum, not an AI agent.
World contains other collections (such as provinces and caravans), omitted here.

```mermaid
classDiagram
    class Simulation {
        -World world
        -u64 current_turn
        -PathFinder finder
        -BTreeMap pending_decisions
        +new(world)
        +queue_decision(settlement_id, decision)
        +step() SimulationTurnReport
        +world() World
        +current_turn() u64
    }
    class World {
        -SettlementRegistry settlements
        -HashMap province_settlements
        -TerritoryManager territory
        +add_settlement(province_id, settlement)
        +inspect_tile(position) TileView
    }
    class Settlement {
        -SettlementId id
        +queue_construction(...)
        +queue_upgrade(...)
        +cancel_project(...)
        +process_turn(...)
    }
    class TerritoryManager {
        -HashMap tiles
        +reserve_construction(...)
        +finish_construction(...)
        +cancel_construction(...)
    }
    class SettlementRegistry {
        -BTreeMap settlements
        -SettlementId next_id
        +get(id) Settlement
        +iter()
        +len() usize
    }
    class PathFinder {
        -Vec nodes
    }
    class SettlementDecision {
        <<enumeration>>
        Construct
        Upgrade
        CancelProject
        SetBuildingLoad
        SetBuildingActive
        SetStorageOrder
    }
    class SimulationTurnReport {
        +u64 turn
        +Vec decisions
        +BTreeMap settlements
        +Vec execution
    }
    Simulation "1" *-- "1" World
    Simulation "1" *-- "1" PathFinder
    Simulation "1" *-- "0..*" SettlementDecision : pending commands
    World "1" *-- "1" SettlementRegistry
    SettlementRegistry "1" *-- "0..*" Settlement : ascending SettlementId
    World "1" *-- "1" TerritoryManager
    Settlement ..> TerritoryManager : borrows for construction
    Simulation ..> Settlement : invokes global phases
    Simulation ..> SimulationTurnReport : returns on success
```

## Lifecycle

Configure a world, register settlements, seed inventories and add initial buildings
using the existing world/settlement commands. Then transfer it into
`Simulation::new(world)`. Alternatively register empty settlements through
`Simulation::add_settlement` before stepping. World registration still enforces
one province per settlement and at most one settlement per province.

The constructor validates map dimensions, buffer lengths, finite terrain values and
unique/existing province IDs before creating the pathfinding buffer. Turn zero
means no global turns have completed. Each successful `step()` returns a report
for turn `current_turn + 1` and increments the counter exactly once, including
for a world with no settlements.

## Settlement registry

`Simulation` owns its single `SettlementRegistry` through `World`. The registry
stores the actual persistent settlement instances in
`BTreeMap<SettlementId, Settlement>`. Province-to-settlement links are a spatial
index, not copies of economic state. `Simulation::settlement_registry()` and
`World::settlement_registry()` expose the same registry by shared reference.

`SettlementId(pub u32)` is distinct from `BuildingId`, `ProjectId` and province
indices. Settlement constructors, world commands, queued decisions, reports,
tile references and caravan participant fields use this type. The caravan
constructor still only builds a route; it does not validate participants against
the registry or participate in global stepping.

Two initialization paths are supported:

- `add_settlement(province_id, settlement)` registers a manually initialized
  settlement with an explicit `SettlementId`, including seeded inventory.
- `initialize_settlement(province_id, capacity, population, needs)` creates and
  registers an empty settlement and returns its automatically allocated ID.

Automatic IDs start at zero and skip registered explicit IDs without wrapping.
Registration failures (including occupied/missing provinces) do not consume an
ID. Duplicate IDs preserve the existing instance and both spatial/economic state.
There is no removal command or identifier reuse during a world's lifetime.

`Simulation::settlement(id)` returns `MissingSettlement(id)` for unknown IDs;
`SettlementRegistry::get(id)` and `World::settlement(id)` provide optional lookups.
No public mutable registry access can bypass province or occupancy validation.
Economic/spatial updates continue through the existing settlement commands.

Registry iteration, world/simulation settlement iteration, report keys and phase
processing all follow ascending `SettlementId`; `step` no longer sorts a hash-map
snapshot. Recovering the world through `into_world` preserves settlement instances
and ID allocation, even though a new coordinator's turn counter starts at zero.

## Global phases

All registered settlements finish one phase before any settlement enters the next.
Every phase visits settlement IDs in ascending order, independent of registration
order. The registry supplies this stable order directly. Within a settlement,
decisions retain submission order.

| Phase | Work |
| --- | --- |
| `Decisions` | Apply queued construction, upgrade, cancellation, building-load/activity and storage-order commands. |
| `Preparation` | Refresh A* productivity, snapshot opening stock, reject invalid projects, refresh labour and reserve prototype trade resources. |
| `Allocation` | Gather population, project and building demand; allocate resources once from existing stock. |
| `Consumption` | Evaluate population consumption and shortages using those allocations. |
| `Construction` | Pay remaining project costs and advance project time exactly once. |
| `Production` | Execute existing buildings using their allocated inputs and labour. |
| `EconomicUpdates` | Collect output/returns, account for overflow, snapshot closing stock, finalize projects and return prototype trade resources. |

```mermaid
flowchart TD
    Step["Caller invokes step()"] --> Validate{"Counter and all existing project memberships valid?"}
    Validate -->|No| Error["Return SimulationError; preserve world, counter and queued decisions"]
    Validate -->|Yes| Decisions
    subgraph Phases["Each phase runs for ALL settlements in ascending ID order"]
        Decisions["Decisions: apply queued commands; record individual rejections"]
        Preparation["Preparation: productivity, labour, project validation and opening stock"]
        Allocation["Allocation: distribute existing resources"]
        Consumption["Consumption: evaluate population allocations"]
        Construction["Construction: pay costs and advance project time"]
        Production["Production: execute existing buildings"]
        Updates["EconomicUpdates: collect output, record losses and finalize projects"]
        Decisions --> Preparation --> Allocation --> Consumption --> Construction --> Production --> Updates
    end
    Updates --> Counter["Increment current_turn exactly once"]
    Counter --> Report["Return SimulationTurnReport"]
```

The arrows connect whole global phases, not consecutive operations for one
settlement. An invalid individual decision is recorded inside the first phase;
it does not take the preflight failure branch. Finalizing construction after
production prevents a new building or upgraded definition from producing early.

This preserves existing settlement economics. New output is unavailable to demands
already allocated in this turn. New buildings and upgraded definitions are applied
after production and can affect production only in a later turn. New buildings
retain the existing default load of zero until configured.

`Settlement::process_turn` remains a local convenience entry point, invoking the
same phase methods in the same order. During global execution, `Simulation::step`
calls those methods across all settlements rather than executing each complete
local turn sequentially. Intermediate phase state is transient and private to
the crate; it is not a second persistent simulation model.

## Decisions and failures

`queue_decision(settlement_id, SettlementDecision)` only queues a command; it does
not change the world immediately. Unknown settlements are rejected when queueing.
The decision type supports `Construct`, `Upgrade`, `CancelProject`,
`SetBuildingLoad`, `SetBuildingActive` and `SetStorageOrder`.

Before consuming queued decisions, `step` checks counter overflow and validates
all existing settlement projects' province membership. A preflight error preserves
the turn counter, pending decisions and world state, including settlements that
would otherwise run before the invalid one. There is no partially completed
recoverable turn.

An individual invalid decision is a different outcome: it is rejected using the
existing command validation, recorded in `DecisionOutcome`, and subsequent commands
and economic phases still run. Successful commands are not rolled back because
another command was rejected. Decisions are consumed once, not replayed each turn.

The coordinator exposes only shared world access while running. `into_world`
recovers the world for external scenario edits and discards queued decisions;
creating another coordinator starts its counter at zero. This is not a checkpoint
or resume API. There is no world rollback for panics or process failures.

## Reports

`SimulationTurnReport` contains the completed turn number, decision outcomes,
settlement `TurnReport`s in a `BTreeMap` and `PhaseExecution` entries recorded at
actual phase invocations. These entries make global ordering inspectable.
The existing inventory accounting boundary is unchanged: it does not cover
prototype trade reservations/returns. See [economy.md](economy.md).

## Headless execution

Run the included deterministic example without initializing graphics:

```sh
cargo run -p simulation_core --example headless -- 100
```

The example builds a small farm scenario and calls `step()` in a loop, dropping
each report after the turn instead of retaining an unbounded history. The optional
argument is the number of turns; the default is 10.

## Validation and scope

Tests cover turn progression, empty worlds, global phase ordering, insertion-order
independence, economic equivalence with local turns, delayed output availability,
construction/upgrade completion, decisions, cancellation, preflight failures and
counter overflow. Unit tests remain alongside their implementation; public API
tests live in `core/tests/simulation_tests.rs`.

AI policies, automatic decision generation, graphics integration, caravan travel,
trade matching, checkpointing and world generation seed reproducibility are not
introduced by this coordinator. It makes the current settlement economy runnable
through a shared global lifecycle.
