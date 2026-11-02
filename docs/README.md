# Documentation

Start with the [architecture guide](architecture/README.md), then move from the
system overview to functional areas and implementation references.

| Directory | Contents |
| --- | --- |
| `architecture/overview/` | System scope, components, aggregate ownership and global turn lifecycle |
| `architecture/systems/` | Rules, classes and lifecycle diagrams for one functional area |
| `architecture/implementation/` | Source map, API contracts, formulas and configuration |
| `decisions/` | Short records of architectural choices and their rationale |
| `planning/` | Proposed behavior, separate from the implemented system |
| `analysis/` | Dated reviews reflecting their original code revision |
| `thesis/` | Thesis requirements and prioritization |

## Recommended reading order

1. [System overview](architecture/overview/system.md).
2. [Components](architecture/overview/components.md) and [aggregate ownership](architecture/overview/state_ownership.md).
3. [Global turn lifecycle](architecture/overview/turn_lifecycle.md).
4. [Settlements](architecture/systems/settlements.md), [registry](architecture/systems/settlement_registry.md), [territory](architecture/systems/territory.md) and [economy](architecture/systems/economy.md).
5. [Project structure](architecture/implementation/project_structure.md) and [source map](architecture/implementation/code_map.md) when locating implementation details.

## Architecture references

- [Architecture and diagram index](architecture/README.md)
- [Buildings](architecture/systems/buildings.md)
- [Trading and caravans](architecture/systems/trading.md)
- [World generation](architecture/systems/world_generation.md)
- [Pathfinding](architecture/systems/pathfinding.md)
- [Rendering and camera](architecture/systems/rendering.md)
- [Headless execution and simulation API](architecture/implementation/simulation_commands.md)
- [Configuration](architecture/implementation/configuration.md)
- [Building productivity](architecture/implementation/building_productivity.md)

## Architectural decisions

- [Economy rules](decisions/0001-economy.md)
- [Code organization](decisions/0002-code-organization.md)
- [Functional modules and spatial dependencies](decisions/0003-functional-modules.md)
- [Map-independent settlement](decisions/0004-settlement-spatial-boundary.md)
- [World and blueprint encapsulation](decisions/0005-world-blueprint-encapsulation.md)

## Plans and proposals

- [Target simulation aggregates](planning/aggregates.md)

## Thesis requirements

- [Thesis MoSCoW priorities](thesis/moscow.md)

## Historical reviews

Historical documents describe their original revision. Source paths and findings
may differ from the current implementation.

- [Historical review: 2026-10-13](analysis/2026-10-13.md)
