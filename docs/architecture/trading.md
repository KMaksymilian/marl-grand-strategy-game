# Trading and Caravans

Trade runs on the settlement instances registered in `World`. It does not keep
another list of economic state. The graphical executable still renders the map
and does not start economic turns automatically.

## World turn

`TradeNetwork::process_turn(&mut World, &HashMap<u32, Point>, &mut PathFinder)`
returns `Result<Vec<TurnReport>, WorldError>`; reports follow ascending settlement ID.
The locations map supplies explicit route endpoints. Missing endpoints or routes
return funded cargo to the sender for unloading in a later turn.

1. Validate every registered settlement's project/province membership before mutation.
2. Move caravans, unload arrivals at home and turn back visitors to absent receivers.
3. Advance each registered settlement with the same global occupancy registry and
   its arrived visitors, sorted by sender ID.
4. Dispatch funded departures after all settlement turns finish.

Province membership, building placement and construction reservations are checked
by the world/settlement lifecycle. A caller cannot pass a different settlement
slice to the network. Local `World::process_settlement_turn` serves existing
visitors but does not move or dispatch caravans globally.

## Settlement trade lifecycle

Published offers persist until replaced. Each turn funds the market first, then
queued caravan orders, and executes market trades with eligible visitors. Material
allocation and construction still use the remaining opening stock. Purchased,
unsold and returned caravan goods are collected with production, so storage order
and warehouse capacity determine admission and reported losses. New products are
not available to earlier allocation or trade in the same turn.

`InventoryTurnReport` includes `trade_reserved` and `trade_received` in its balance
equation. `TradeTurnReport` records market executions, departures, returns, gold
and unfunded orders. Gold is separate from warehouse material capacity.

`Settlement` exposes publishing/clearing offers, ordering/canceling departures,
receiving returns and reading trade state. `TradeBook` handles bilateral and shared
market matching; `TradeAllocationEngine` reserves goods and treasury funds.

## Caravans

A caravan carries a funded book and participant IDs, follows an A* route and stores
`TravelTo`, `Trading`, `TravelBack` or `Back`. Movement uses the existing pathfinding
rules. `Caravan::from_cargo` returns the cargo on routing failure; an undelivered
return records `NoRoute` or `ReceiverMissing`. Eligible visitors trade only at their
receiver; returning cargo is unloaded into its sender's registered settlement.

## Sources and tests

- [Network entry point](../../core/src/trade/network.rs)
- [World transport coordination](../../core/src/world/state/trade.rs)
- [Local trade phases](../../core/src/settlement/trade_lifecycle.rs)
- [Trade commands](../../core/src/settlement/commands/trade.rs)
- [Spatial integration](territory.md#integration-with-trade)
- Network unit tests cover trade, shared markets, movement, failed routes and overflow.
- Territory/trade integration tests cover construction, cancellation and accounting
  against the same persistent settlement state.
