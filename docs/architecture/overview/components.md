# Components and Dependencies

## Dependency view

An arrow means a code dependency or composition call, not a running network service.

```mermaid
flowchart TD
    App[main_app: graphical composition] --> Adapter[adapters: view and input]
    App --> Core[simulation_core]
    App --> MQ[Macroquad]
    Adapter --> Core
    Adapter --> MQ
    Example[core/examples/headless.rs] --> Core
    subgraph Modules[Functional modules inside simulation_core]
        Sim[simulation: global lifecycle] --> World[world: registry and spatial state]
        Sim --> Settlement[settlement: local lifecycle]
        World --> Settlement
        World --> Navigation[pathfinding and terrain]
        Settlement --> Buildings[buildings]
        Settlement --> Economy[economy]
        Settlement --> Trade[trade books and funding]
        Settlement --> Spatial[spatial: values and construction contract]
        Buildings --> Costs[TravelCostSource contract]
        World --> Environment[map adapters]
        Environment --> Navigation
        Environment --> Spatial
        Environment --> Costs
        Buildings --> Economy
    end
    Core --> Sim
```

The module view highlights runtime cooperation; it is not a complete import graph.


## Responsibilities

| Component | Owns or coordinates | Does not provide |
| --- | --- | --- |
| `Simulation` | Global time, pending decisions, phase order and owned world | Automatic policies or graphical frame timing |
| `World` | Settlement registration, province index and global occupancy | Economic allocation formulas |
| `Settlement` | Local economic state and project effects | Another private copy of the global tile registry |
| `SettlementRegistry` | One persistent instance per typed ID; sorted iteration | Public insertion that could bypass province validation |
| `adapters` | Camera, textures, input and overlays | Simulation ownership or turn advancement |
| `main_app` | Graphical initialization and frame loop | Headless economic orchestration |

No HTTP API, Python bridge, message broker or distributed runtime exists in this
architecture. [Project structure](../implementation/project_structure.md) and the
[source map](../implementation/code_map.md) locate the implementing files.
