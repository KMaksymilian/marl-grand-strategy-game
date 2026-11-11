# Settlement Dependency Boundaries

`Settlement` owns local economic state and the project lifecycle. Its runtime
implementation and the buildings module import no `world` types.

Map independence here means independence from the concrete spatial implementation,
not the absence of spatial data. A building retains an immutable `spatial::Position`
and construction retains a target position.



## Contracts and adapters

```mermaid
classDiagram
    class ConstructionSpace {
        <<interface>>
        +reserve_construction()
        +validate_construction()
        +cancel_construction()
        +finish_construction()
        +register_building()
        +release_rejected_construction()
    }
    class TravelCostSource {
        <<interface>>
        +travel_cost(from, to) Option~u32~
    }
    Settlement *-- BuildingManager
    Settlement ..> ConstructionSpace : project lifecycle
    BuildingManager ..> TravelCostSource : productivity refresh
    MapConstructionSpace ..|> ConstructionSpace
    AStarTravelCosts ..|> TravelCostSource
    MapSettlementEnvironment ..|> ConstructionSpace
    MapSettlementEnvironment ..|> TravelCostSource
    MapSettlementEnvironment *-- MapConstructionSpace
    MapSettlementEnvironment *-- AStarTravelCosts
    MapConstructionSpace ..> Map : validates land
    MapConstructionSpace ..> TerritoryManager : borrows occupancy
    AStarTravelCosts ..> Map : borrows terrain
    AStarTravelCosts ..> PathFinder : borrows search buffers
    World ..> MapSettlementEnvironment : creates temporary adapter
    World *-- SettlementRegistry
    SettlementRegistry *-- Settlement
```



| Operation | Required capability |
| --- | --- |
| Queue or cancel construction | `ConstructionSpace` |
| Configure productivity | `TravelCostSource` |
| Register a building or execute a local turn | Both |
| Deposit, load/activity settings, storage order, queue upgrade | Neither |


## Command flow

```mermaid
sequenceDiagram
    participant Sim as Simulation
    participant W as World
    participant S as Settlement
    participant E as MapConstructionSpace
    participant T as TerritoryManager
    Sim->>W: Dispatch a validated command through the application handler
    W->>W: resolve settlement and validate province
    W->>S: queue_construction(..., space)
    S->>S: accept validated definition and check local project ID
    S->>E: reserve_construction(position, qualified project)
    E->>E: validate map bounds and land
    E->>T: reserve empty structural slot
    T-->>E: result
    E-->>S: result
    S-->>W: register project on success
    W-->>Sim: command outcome
```
