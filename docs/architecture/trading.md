# Trading and Caravans

Status: domain prototype, introduced on develop before the code reorganization.
The executable does not create settlements/offers/caravans or drive their turns.
Source responsibilities and method names are listed in the [code map](code_map.md).

## Data and execution

`GoldInventory` stores `u32` currency separately from warehouse materials.
`TradeLog` owns reserved gold and one `TradeOffer` per resource. Sell offers
describe available stock and a minimum price; buy offers describe remaining demand,
purchased stock and a maximum price. `calculate_demand()` requests materials for
sales and quantity multiplied by the maximum price for purchases.

`Settlement::process_turn()` asks `TradeAllocationEngine` to reserve these goods
and funds before local population/building/project allocation. At the end of the
turn it drains purchased goods, unsold stock and remaining gold back into the
settlement inventories. A newly constructed settlement has an empty trade log;
there are currently no public settlement commands to configure it.

`TradeLog::trade()` matches buy/sell offers in two logs when the seller's minimum
price does not exceed the buyer's maximum. Unit prices use their integer midpoint.
When funds are insufficient, the transaction scales quantities proportionally,
then spends remaining funds on affordable quantities. This is a bilateral matching
implementation, not a global market/order-book service.

`Caravan` lives in `domain::trade::caravan`. It carries a log and follows a stored
A* route between province centroids. Its state progresses through `TravelTo`,
`Trading`, `TravelBack` and `Back`. `trade()` must be called externally on arrival.
`World` owns the caravan collection; `caravan_travel()` advances movement only.
Sender and receiver IDs currently index provinces, not a settlement registry.

## Current limitations

The organization change moves files and updates imports; it does not revise
trade behavior. In particular:

- Offer fields are private and have no public constructors. `TradeLog::new()`
  accepts a map of offers, but external callers cannot construct populated offer
  values through a supported constructor. There are no settlement market commands.
- Gold and several price/quantity operations use unchecked `u32` arithmetic.
- Reserved trade goods, purchases and trade overflow are not represented in
  `InventoryTurnReport`. Its opening snapshot precedes reservations, and its
  closing snapshot precedes trade returns. `is_balanced()` therefore does not
  establish conservation for turns with nonempty trade activity.
- Returned goods use direct inventory insertion; rejected quantities are ignored
  and do not follow the settlement manager's ordered batch admission/reporting.
- `Caravan::new()` unwraps pathfinding and can panic for an unreachable route;
  `travel_back()` subtracts the movement step before checking the lower bound,
  which can underflow near the route origin.
- Routes are fixed after creation. No rerouting, cargo capacity, arrival dispatch,
  finished-caravan removal or global turn scheduling is implemented.

The [economy roadmap](../planning/economy_roadmap.md) describes intended future
logistics. Its proposals should not be read as completed functionality.
