# Settlement Decision Contract

`SettlementDecision` is the typed, serializable representation of what one
settlement agent wants to do in one turn. Rule-based and learning agents
(including Python agents through JSON) submit the same values. Source:
[application/commands/settlement/contract.rs](../../../core/src/application/commands/settlement/contract.rs).

Status: the contract, its JSON format, validation and execution are implemented;
`Simulation::queue_decision` takes a `SettlementDecision`. Agents read the state
through [observations](settlement_observations.md) and receive per-action outcomes
in the completed turn report.

## Types

```rust
pub struct SettlementDecision {
    pub settlement_id: SettlementId,
    pub actions: Vec<SettlementAction>, // applied in list order
}
```

| `SettlementAction` | Parameters | Meaning and constraints |
| --- | --- | --- |
| `Construct` | `building_type`, `position` | Build a catalog building on an empty land tile of the own province. The simulation assigns the `ProjectId`; the cost comes from the catalog. |
| `Upgrade` | `building_id`, `building_type` | Replace an own building's definition with another catalog building. No upgrade paths are enforced yet. |
| `CancelProject` | `project_id` | Stop an own active project; no refund. |
| `SetBuildingWorkers` | `building_id`, `workers` | Requested staff in `0..=max_workers`. Zero switches the building off; recipe inputs scale with the staff. |
| `SetStorageOrder` | `order` | Every material exactly once, no `Labour`. |
| `PublishMarketOffers` | `offers` | Replace all market offers. Offers persist until replaced or cleared. |
| `ClearMarketOffers` | none | Remove all market offers; the market book stays empty and visiting caravans find nothing to trade. |
| `OrderCaravan` | `receiver_id`, `offers` | Reserve goods and gold for a caravan to another settlement. The order is realized at the end of the settlement turn. |

`OfferSpec { resource, side: buy|sell, units, price_per_unit }` holds only order
parameters. The price is the minimum when selling and the maximum when buying.
It never contains purchase progress, so a submitted offer cannot carry goods.

`ActionKind` is the parameterless variant (`SettlementAction::kind()`), with
`ActionKind::ALL` in a fixed order.

Agents never supply costs, building definitions or project IDs. Building types
are referenced by `BuildingTypeId`, the catalog key.

## Validation

`validate_input(action)` checks submitted fields without a world reference.
Negative worker counts, units and prices fail JSON parsing because those
fields are unsigned. The input validator rejects negative coordinates,
malformed offers and empty caravan orders.

During the Decisions phase, the handler checks and runs each action on the
current world, after all earlier actions have finished. A rejected action
changes nothing and leaves its target available. The result contains one entry
per action in input order. An unknown deciding settlement rejects the decision
as a whole. Parsing and queuing are separate from these checks.

Construction and upgrades resolve the catalog definition, its cost and the next
project ID at execution. The domain checks placement and reserves the site in
the occupancy registry. Cancellation releases the reservation or upgrade lock
immediately. The domain also owns staffing, storage order and trade checks.

| Check | Errors |
| --- | --- |
| Identifiers | `UnknownBuildingType` (not in `World::building_catalog()`), `UnknownBuilding`, `UnknownProject`, `UnknownSettlement` (caravan receiver) |
| Placement | `InvalidPosition` (off the map), `OutsideOwnProvince`, `OceanTile`, `TileOccupied` (building or construction site) |
| Upgrades | `SameBuildingType`, `BuildingAlreadyUpgrading` (active upgrade project) |
| Workers | `WorkersAboveCapacity` (`workers > max_workers`), `InsufficientWorkers` (all buildings would request more staff than the population; reductions are always allowed), `UnsupportedWorkerCount` (count no `f32` load expresses; unreachable because definitions reject `max_workers` above `BuildingDefinition::MAX_WORKERS` = 2^24) |
| Storage order | `InvalidStorageOrder` with the engine's `StorageOrderError` (`StorageOrder::validate`) |
| Offers (market and caravans) | `LabourIsNotTradable`, `ZeroUnits`, `DuplicateOffer` |
| Caravans | `EmptyOrder`, `TradeWithSelf`, `UnknownSettlement`, `NothingToCarry` (no goods and a zero purchase budget; the engine would skip it), `InsufficientStock`, `InsufficientGold` (see funding order) |
| Market after caravans | `InsufficientStock`, `InsufficientGold`: a `PublishMarketOffers` may not reserve goods or gold already committed to caravans of this turn |
| Same target twice | `Conflict { with_index }`: the earlier accepted action wins |
| Exhausted IDs | `ProjectIdsExhausted` |

Conflict targets: one tile per construction, one upgrade and one staff change per
building, one cancellation per project, one storage order, and one market action
(`PublishMarketOffers` or `ClearMarketOffers`). Several caravans are allowed.

Ownership: building and project IDs are looked up only in the deciding
settlement, so another settlement's assets are reported as unknown; positions must
be in the own province.

Not checked, by design:

- Construction materials. Projects pay over several turns, so the warehouse does
  not have to hold the full cost when construction starts.
- Market offer stock and gold. Published offers persist and every turn reserves
  what is available at that time.

**Funding order.** Admission gives the currently published market priority and
reserves the accepted order's full goods and gold. Later orders and market
changes see the remaining resources, including reservations from earlier
decisions of the same settlement. At the end of the settlement turn the orders
are realized in FIFO order and handed to transport. `InsufficientStock` and
`InsufficientGold` show the amount requested and what remained available.

Errors serialize as `{"index": 1, "kind": "construct", "error": {"reason":
"tile_occupied", "position": {"x": 0, "y": 0}}}`.

## Execution

The [response contract](../../../core/src/application/commands/settlement/response.rs)
defines an ordered `ActionResult` for each action. An applied construction
returns `ProjectQueued { project_id }`. A rejected construction returns a
reason such as `TileOccupied` and the position.


In the Decisions phase, `Simulation::step` runs nation decisions first and then
settlement decisions. Each successful mutation is visible to the next action.
Effects such as new staff apply to the same turn's allocation.
[Handler source](../../../core/src/application/commands/settlement/handler.rs)

| Action | Settlement operation | Effect reported |
| --- | --- | --- |
| `Construct` | `queue_construction` with catalog definition and cost; reserves the tile | `ProjectQueued { project_id }` |
| `Upgrade` | `queue_upgrade` with catalog definition and cost | `ProjectQueued { project_id }` |
| `CancelProject` | `cancel_project`; releases a site or upgrade lock | `ProjectCanceled { project_id }` |
| `SetBuildingWorkers` | `set_building_staff`: activates the building and sets the exact load | `WorkersSet { building_id, workers }` |
| `SetStorageOrder` | `set_storage_order` | `StorageOrderSet` |
| `PublishMarketOffers` | `publish_trade_offers` | `MarketOffersPublished` |
| `ClearMarketOffers` | `clear_trade_offers` | `MarketOffersCleared` |
| `OrderCaravan` | `order_caravan`; reserves cargo and gold at acceptance | `CaravanOrdered { receiver_id }` confirms acceptance. The trade report records realization and routing. |

Result JSON:

```json
{"settlement_id": 1, "results": [
  {"index": 0, "kind": "set_building_workers",
   "outcome": {"status": "applied",
               "effect": {"effect": "workers_set", "building_id": 1, "workers": 2}}},
  {"index": 1, "kind": "construct",
   "outcome": {"status": "rejected",
               "error": {"reason": "outside_own_province", "position": {"x": 3, "y": 0}}}}
]}
```

The handler reports the effect after the command succeeds.

## Design notes and open questions

- **Market before population (intended).** Market and caravan reservations take
  goods from the warehouse before population and building allocation. Offering all
  food can starve the settlement in that turn; that is the agent's choice.
- **Workers versus project labour (open).** `InsufficientWorkers` and the mask
  compare building staff with the population only. A project whose catalog cost
  includes `Labour` competes for the same labour in allocation, so a building can
  receive fewer workers than validated. No current definition does this.
- **Decision-level rules in masks.** Masks describe single actions; conflicts and
  shared caravan limits within one decision are enforced only by the validator.
- **Python side.** Decisions, results and observations are JSON values; the
  Python bridge and the encoding into fixed-size vectors belong to the Python side.

## JSON format

```json
{
  "settlement_id": 1,
  "actions": [
    { "type": "construct", "building_type": "farm", "position": { "x": 3, "y": 4 } },
    { "type": "upgrade", "building_id": 2, "building_type": "big_farm" },
    { "type": "cancel_project", "project_id": 5 },
    { "type": "set_building_workers", "building_id": 2, "workers": 3 },
    { "type": "set_storage_order", "order": ["Grain", "Wood", "Stone"] },
    { "type": "publish_market_offers", "offers": [
        { "resource": "Wood", "side": "sell", "units": 10, "price_per_unit": 2 } ] },
    { "type": "clear_market_offers" },
    { "type": "order_caravan", "receiver_id": 2, "offers": [
        { "resource": "Stone", "side": "sell", "units": 6, "price_per_unit": 1 } ] }
  ]
}
```

- IDs are plain integers and `building_type` is a plain string.
- Action tags and offer sides use `snake_case`; resource identifiers use
  upper camel case (`Wood`, `Stone`, `Grain`, `Labour`, `RockSalt`).
- A missing `actions` list means no actions.
- Parsing rejects unknown fields and tags, missing fields, negative or
  fractional quantities, and malformed resource identifiers. Action validation
  checks material identifiers against the settlement's catalog. It also rejects
  `Labour` in an offer.

`SettlementDecision::to_json()` and `SettlementDecision::from_json(&str)`
convert to and from text. `from_json` returns `DecisionParseError`, whose
message starts with `invalid settlement decision` and includes the JSON
position of the problem.
