# Pathfinding

The map uses an eight-neighbor land grid. Map overlays, caravan route creation
and building productivity share its movement rules. Costs are directed: an uphill
journey can cost more than the reverse journey.

| Query | Result | Current caller |
| --- | --- | --- |
| `AStar::a_star` | Optional point path | Map overlay and caravan construction |
| `AStar::travel_cost` | Optional minimum route cost | Building productivity |
| `DijkstraFlood::dijkstra_flood` | Points reachable within a budget | Reachable-area overlay |

## Route query sequence

```mermaid
sequenceDiagram
    actor Caller
    participant A as AStar
    participant M as Map
    participant F as PathFinder
    Caller->>A: a_star(map, finder, start, end)
    A->>A: travel_cost using the same search
    A->>M: validate shape, endpoint bounds and land
    alt Invalid input or ocean endpoint
        A-->>Caller: None
    else Valid query
        A->>F: begin new generation
        loop Explore until goal or open set exhausted
            A->>M: land neighbors and directed movement costs
            A->>F: relax best cost and parent
        end
        alt Goal reached
            A->>F: reconstruct path
            A-->>Caller: Some point path
        else Unreachable
            A-->>Caller: None
        end
    end
```

`travel_cost` callers skip reconstruction and its path allocation. A mutable
`PathFinder` is reusable scratch storage for one fixed map shape, not a route
service with concurrent searches.

Exact costs, octile heuristic, overflow behavior and verification are in
[pathfinding algorithms](../implementation/pathfinding_algorithms.md).
