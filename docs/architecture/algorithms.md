# Pathfinding

The map is an eight-neighbor grid. `AStar`, `DijkstraFlood` and building
productivity share `PathFinder` movement rules. Coordinates are `(usize, usize)`
or `Point`; flat indices use `y * width + x`. See the [code map](code_map.md) for
every method and source link.

## Shared search state

`PathFinder` allocates a private `Vec<Node>` for fixed map dimensions. Each node
stores its best cost, parent index and seen/closed generation markers. `begin()`
starts a search and initializes the start node. Old nodes are ignored by their
generation rather than clearing the full buffer each time. At `u32::MAX`, nodes
are cleared and the generation restarts at one.

`try_relax()` records improvements, `g_score()` reads the current cost and
`path_to()` reconstructs a `Vec<Point>`. The heap belongs to each search; it is
not part of the reusable buffer. Searches sharing a mutable buffer run sequentially.

## Movement costs

Straight steps cost 10 and diagonal steps 14 before terrain penalties. Ocean
tiles (`elevation < 0.2`) are excluded. Diagonal steps do not test whether the
two adjacent orthogonal tiles are ocean.

For elevation difference `d`, step length `s`, slope scale 1000 and uphill factor
1.5, the implementation computes:

```text
ratio = abs(d) * 1000 * (10 / s)
if d > 0: ratio *= 1.5
cost = (s * (1 + ratio * ratio)) as u32
```

The ratio uses floating-point arithmetic. Costs are directed: climbing can cost
more than descending. Accumulated search costs use saturating addition.

## A*

`AStar::travel_cost()` returns `Option<u32>` without allocating a reconstructed
path. It validates map size, endpoint bounds, terrain length and pathfinder shape,
then rejects ocean endpoints. `AStar::a_star()` calls that same search and uses
`path_to()` to return `Option<Vec<Point>>`. The heuristic is the octile distance:

```text
14 * min(dx, dy) + 10 * (max(dx, dy) - min(dx, dy))
```

The arithmetic saturates to avoid heuristic overflow. The view and caravans use
point paths; building productivity uses only the cost.

## Dijkstra flood

`DijkstraFlood::dijkstra_flood()` returns `Vec<Point>` reached within an explicit
movement budget. It shares neighbor enumeration, movement costs and generation
state with A*. Unlike A*, its entry point currently assumes valid map buffers,
an in-bounds start and matching pathfinder dimensions; callers must ensure these.

## Verification

Inline tests compare A* cost with reconstructed paths, check slopes, detours,
disconnected/ocean/invalid endpoints and repeated buffer use. A pathfinder unit
test exercises generation wrap across A* and Dijkstra. Integration tests cover
the effect of changed map routes on settlement productivity.
