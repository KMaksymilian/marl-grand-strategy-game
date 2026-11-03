# Settlements

A `Settlement` coordinates its population, inventories, buildings and projects.
Its economic state persists between turns in the world's single registry. It is
not itself an implemented AI policy or a separate borrowed API object.

## Local class view

```mermaid
classDiagram
    class Settlement {
        -SettlementId id
        +inventory()
        +buildings()
        +active_projects()
        +queue_construction()
        +queue_upgrade()
        +cancel_project()
    }
    Settlement *-- SettlementInventoryManager
    Settlement *-- BuildingManager
    Settlement *-- PopulationManager
    Settlement *-- ProjectManager
    Settlement *-- TradeManager
    Settlement *-- TradeBook
    SettlementInventoryManager *-- ResourceInventory : warehouse and buffer
    BuildingManager *-- Building
    ProjectManager *-- Project
    PopulationManager o-- NeedRegistry : shared needs
    Project *-- ProjectCost : remaining resources and turns
    Project o-- BuildingDefinition : requested result
```

Gold, market templates, funded books and caravan order/return queues are also settlement-owned; see the full
[aggregate view](../overview/state_ownership.md). There is no public mutable
manager/collection getter. Read access and validated commands remain on `Settlement`.
Global callers use queued decisions or world entry points so spatial validation
uses the authoritative registry.

## Project lifecycle

These states are derived from `remaining_cost` and active-list membership, not a
stored project status enum. Construction and upgrades run concurrently across
active projects; there is no one-project-at-a-time construction queue.

```mermaid
stateDiagram-v2
    [*] --> Queued: valid command accepted
    Queued --> InProgress: resources or turns remain
    Queued --> ReadyToFinalize: zero remaining cost
    InProgress --> InProgress: turn advances, grants may pay materials
    InProgress --> ReadyToFinalize: all materials and time paid
    Queued --> Canceled: cancel_project
    InProgress --> Canceled: cancel_project
    ReadyToFinalize --> Canceled: canceled before finalization
    InProgress --> Rejected: invalid effect or construction site
    ReadyToFinalize --> Completed: finalization succeeds after production
    ReadyToFinalize --> Rejected: finalization fails
    Completed --> [*]
    Canceled --> [*]
    Rejected --> [*]
```

A rejected queue command never enters this lifecycle. Time currently decreases
without material delivery, and completion waits for both time and material costs.
`RecruitUnit` is explicitly rejected; no unit is created. Command costs are caller supplied; blueprint `construction_cost` is not applied
automatically. Already paid materials are not refunded on rejection/cancellation.

Completion/rejection during a turn produces a `ProjectOutcome`; pending projects
produce none. Explicit cancellation returns immediately and has no later turn
event. Completed project history is not retained; inactive project numbers may be reused.

## Cancellation sequence

```mermaid
sequenceDiagram
    actor Caller
    participant W as World
    participant S as Settlement
    participant T as ConstructionSpace adapter
    Caller->>W: cancel_project(settlement_id, project_id)
    W->>S: cancel_project(project_id, space)
    S->>S: find active project
    alt Missing project
        S-->>W: MissingProject, no mutation
    else Project found
        alt New construction
            S->>T: cancel_construction(position, qualified project reference)
            T-->>S: exact reservation released or mismatch error
        else Upgrade
            Note over S,T: Keep the existing building and occupancy
        end
        alt Reservation valid or no reservation needed
            S->>S: remove active project, retain previously spent cost
            S-->>W: Ok
        else Reservation mismatch
            S-->>W: ProjectError, project and occupancy unchanged
        end
    end
    W-->>Caller: result
```

Registration and identities are described in [registry](settlement_registry.md).
Spatial reservations are described in [territory](territory.md). The
[global turn](../overview/turn_lifecycle.md) schedules shared settlement phases.
Exact local API, time/cost examples and report contracts are in
[settlement commands](../implementation/settlement_commands.md).
