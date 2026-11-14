# Territory and Tile Inspection

`World` owns one global `TerritoryManager`. World entry points resolve the settlement and validate province membership, then delegate lifecycle changes to that settlement.



## Spatial data

| Source | Data |
| --- | --- |
| `Map` | Terrain, the province ID of each grid position and sparse material deposits |
| `World::province_settlements` | Optional settlement registered in a province |
| `World::settlements` | One `SettlementRegistry` storing instances in ascending `SettlementId` order |
| `TerritoryManager::tiles` | Sparse `HashMap<Position, Tile>` of structural occupancy |
| `Settlement` | Actual building instances and active projects |

A registered settlement belongs to exactly one province. A province contains at
most one settlement and may remain empty.

`World::controlled_positions(settlement_id)` derives territory from the
province layer and returns positions in row-major order. The result includes
water tiles. `World::accessible_deposits(settlement_id)` returns the deposits
on land within those positions.

Scenario setup places a single material deposit per tile with
`World::place_deposit`. For example, place a `Stone` deposit and register a
quarry blueprint with `requiring_deposit(ResourceType::Stone)` and a production
recipe that outputs stone. Deposits are permanent and independent of structural
occupancy. A flooded deposit is still visible in inspection, but extraction
requires land. Each turn, a building whose required deposit is inaccessible
requests zero inputs and workers. It can resume when access is restored.

`TileOccupancy` is `Empty`, `Construction(ProjectReference)` or
`Building(BuildingReference)`. References include both settlement ID and local
project/building ID.
## Construction and cancellation

```mermaid
stateDiagram-v2
    Empty --> Construction: queue construction
    Construction --> Building: complete paid project
    Construction --> Empty: cancel or reject own project
    Empty --> Building: register existing building
    Building --> Building: upgrade or cancel upgrade
```

- `World::queue_construction` checks province membership, land, vacancy and
  the blueprint's required deposit. The settlement validates the project ID
  and reserves the structural slot through its `ConstructionSpace` adapter.
- `World::add_building` applies the same placement checks and registers a completed
  building. It cannot replace a project or another building.
- The global turn coordinator uses the same shared registry for settlement phases.
  Construction is revalidated before allocation and completion. Completion replaces
  the reservation with the actual building reference.
- `World::queue_upgrade` checks access to the target blueprint's required
  deposit. Its structural slot remains a building during the project.
- `World::cancel_project` delegates cancellation to `Settlement::cancel_project`.
  A construction reservation must match the exact project and settlement. A mismatch
  preserves both project and tile. Canceling an upgrade leaves its building intact.

Current building types require land, using `Elevation::OCEAN_LEVEL`.
Cancellation stops further costs; materials already consumed remain spent.
If province membership changes between turns, the world rejects that settlement's
turn before spending, preserving projects and reservations.

## Inspection

`World::inspect_tile(position)` returns a borrowed `TileView` containing:

- Position, terrain reference and optional material deposit.
- Province ID and optional `province_settlement` membership reference.
- Optional `occupant_settlement`, resolved from the qualified structural owner.
- Occupancy snapshot and the referenced building or new-construction project.



Implementation: [registry](../../../core/src/domain/world/territory.rs),
[world commands](../../../core/src/domain/world/state/construction.rs),
[inspection](../../../core/src/domain/world/state/inspection.rs),
[settlement commands](../../../core/src/domain/settlement/model/commands/projects.rs).


## Construction sequence

```mermaid
sequenceDiagram
    actor Caller
    participant W as World
    participant S as Settlement
    participant T as MapConstructionSpace
    Caller->>W: queue_construction(settlement, project, site, definition, cost)
    W->>W: validate province, land, vacancy and required deposit
    W->>S: queue_construction with construction-space port
    S->>S: accept validated definition and check project ID
    S->>T: reserve_construction(site, qualified project reference)
    alt Occupied structural slot
        T-->>S: Occupied error, slot unchanged
        S-->>W: project rejected, no cost paid
        W-->>Caller: error
    else Empty slot
        T-->>S: land and vacancy validated, reserved
        S->>S: register active project
        S-->>W: accepted, no immediate material spending
        W-->>Caller: Ok
    end
    Note over S,T: Subsequent turn allocation pays costs and advances time
    opt Matching project becomes fully paid and valid
        S->>T: finish_construction(site, project reference, building reference)
        T-->>S: slot now holds building
        S->>S: insert completed building
    end
```

## Spatial class view

```mermaid
classDiagram
    World *-- Map
    World *-- TerritoryManager
    World ..> Settlement : province membership
    TerritoryManager *-- Tile : by Position
    Tile *-- TileOccupancy
    class TileOccupancy {
        <<enumeration>>
        Empty
        Construction
        Building
    }
    TileOccupancy --> ProjectReference
    TileOccupancy --> BuildingReference
    ProjectReference --> SettlementId
    ProjectReference --> ProjectId
    BuildingReference --> SettlementId
    BuildingReference --> BuildingId
    World ..> TileView : combines borrowed data
```

Tile lookup combines province membership and terrain from the map with occupancy;
it does not infer deposits from warehouse goods or reveal an AI agent's knowledge.
The current graphical click handler runs pathfinding, not `inspect_tile`.

## Inspection and economic integration

`TileView::province_settlement` describes current province membership;
`occupant_settlement` resolves the settlement-qualified structural reference.
They may differ after province edits. A dangling reference or an object whose
position disagrees with the tile returns `WorldError::InconsistentOccupancy`.
Turn preflight rejects projects outside their registered province before trade
funding, movement or construction spending. Trading and construction use the
same persistent registered settlement instances.
