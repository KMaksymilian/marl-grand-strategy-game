# Settlement Decision Contract

`SettlementDecision` is the typed, serializable representation of what one
settlement agent wants to do in one turn. Rule-based and learning agents
(including Python agents through JSON) submit the same values. Source:
[settlement/decision.rs](../../../core/src/settlement/decision.rs).

Status: the contract and its JSON format are implemented. Validation, execution,
observations and action masks are separate later stages. Until execution is
connected, `Simulation::queue_decision` still takes the older
[`SettlementCommand`](simulation_commands.md#decision-contracts).

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
| `OrderCaravan` | `receiver_id`, `offers` | Send a caravan to another settlement; funded from stock and gold during the turn. |

`OfferSpec { resource, side: buy|sell, units, price_per_unit }` holds only order
parameters. The price is the minimum when selling and the maximum when buying.
It never contains purchase progress, so a submitted offer cannot carry goods.

`ActionKind` is the parameterless variant (`SettlementAction::kind()`), with
`ActionKind::ALL` in a fixed order for mask vectors.

Agents never supply costs, building definitions or project IDs. Building types
are referenced by `BuildingTypeId`, the catalog key.

## Rejection policy

Each action is accepted or rejected on its own. A rejected action changes
nothing and does not stop later actions in the same decision. The validator
(next stage) will report a structured error for each rejected action.

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
- Action tags and offer sides use `snake_case`; resources use their Rust names
  (`Wood`, `Stone`, `Grain`, `Labour`).
- A missing `actions` list means no actions.
- Parsing rejects unknown fields and tags, missing fields, negative or
  fractional quantities, and unknown resources. Parsing does not check values
  against the world: for example, `Labour` in an offer parses and is rejected
  by validation.

`SettlementDecision::to_json()` and `SettlementDecision::from_json(&str)`
convert to and from text. `from_json` returns `DecisionParseError`, whose
message starts with `invalid settlement decision` and includes the JSON
position of the problem.
