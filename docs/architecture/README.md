# Architecture

This section describes the code that exists now. Start with the system view,
choose a functional area, and use implementation references when changing code.
Proposals and decision rationale live in [planning](../planning/aggregates.md)
and [decisions](../decisions/0001-economy.md), respectively.

## 1. Overview: understand the whole system

| Document | Question answered |
| --- | --- |
| [System](overview/system.md) | What runs today, and how do graphical and headless execution differ? |
| [Components](overview/components.md) | Which packages and modules depend on each other? |
| [State ownership and aggregates](overview/state_ownership.md) | Who owns state, and where are consistency boundaries? |
| [Global turn lifecycle](overview/turn_lifecycle.md) | In what order do decisions and economic systems execute? |
| [Settlement dependency boundaries](overview/settlement_boundaries.md) | How does the settlement run independently of map and pathfinding implementations? |

## 2. Systems: understand one area

| Document | Responsibility |
| --- | --- |
| [Settlements](systems/settlements.md) | Local population, inventory and project lifecycle |
| [Settlement registry](systems/settlement_registry.md) | Identification, initialization and deterministic traversal |
| [Economy](systems/economy.md) | Allocation, resource flow, storage and accounting |
| [Buildings](systems/buildings.md) | Definitions, runtime instances, production and upgrades |
| [Territory](systems/territory.md) | Province membership, occupancy and tile inspection |
| [Trading and caravans](systems/trading.md) | Market funding, visitor matching, transport lifecycle and limitations |
| [World generation](systems/world_generation.md) | Terrain and province generation |
| [Pathfinding](systems/pathfinding.md) | Shared movement rules and route queries |
| [Rendering](systems/rendering.md) | Map display, camera and input |

## 3. Implementation: locate contracts and calculations

| Document | Reference |
| --- | --- |
| [Project structure](implementation/project_structure.md) | Packages, directories and import migration |
| [Source file and method map](implementation/code_map.md) | Explicit runtime methods and their files |
| [Simulation commands](implementation/simulation_commands.md) | Coordinator API, decisions, reports and failures |
| [Settlement commands](implementation/settlement_commands.md) | Local commands, project costs and turn outcomes |
| [Economy rules](implementation/economy_rules.md) | Integer formulas, rounding, capacity and satisfaction |
| [Building productivity](implementation/building_productivity.md) | A* cost thresholds and exact output calculation |
| [Pathfinding algorithms](implementation/pathfinding_algorithms.md) | Search state, heuristic and directed movement cost |
| [Configuration](implementation/configuration.md) | TOML parsing, validation and actual application wiring |

## Diagram index

| View | Diagram location |
| --- | --- |
| Components and package dependencies | [Components](overview/components.md#dependency-view) |
| Settlement ports, adapters and command sequence | [Dependency boundaries](overview/settlement_boundaries.md) |
| Main classes and aggregate ownership | [State ownership](overview/state_ownership.md#aggregate-and-ownership-view) |
| Global turn sequence | [Turn lifecycle](overview/turn_lifecycle.md#global-sequence) |
| Turn execution states | [Turn lifecycle](overview/turn_lifecycle.md#execution-state-view) |
| Settlement classes, project states and cancellation | [Settlements](systems/settlements.md) |
| Registration sequence and identity lifecycle | [Registry](systems/settlement_registry.md) |
| Inventory classes and resource activity | [Economy](systems/economy.md) |
| Building classes and operating states | [Buildings](systems/buildings.md) |
| Occupancy states and construction sequence | [Territory](systems/territory.md) |
| Trade ownership, caravan states and global execution | [Trading](systems/trading.md) |
| World generation activity | [World generation](systems/world_generation.md) |
| Route query sequence | [Pathfinding](systems/pathfinding.md) |
| Graphical startup and frame sequence | [Rendering](systems/rendering.md) |
| Configuration loading activity | [Configuration](implementation/configuration.md) |

Class diagrams use composition for owned state, aggregation for shared definitions,
and dependencies for borrowed access. Sequence diagrams describe calls over time.
State diagrams identify actual stored states or explicitly label derived lifecycle
views. Component/activity views use Mermaid flowchart notation. Diagrams simplify
signatures for readability; exact contracts belong to implementation references.

Keep one authoritative explanation of each rule and link to it from other levels.
Add a system subdirectory only when that system needs several separate documents.
Unsupported behavior must be labeled as a limitation, not drawn as a working path.

- [Trading file and method reference](implementation/trading.md)
