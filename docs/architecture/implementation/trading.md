# Trading Implementation Reference

The [system guide](../systems/trading.md) explains ownership, timing, matching,
states and limitations. This reference maps those operations to source files.
Child modules add methods to their owning types rather than extra service objects.

## Offers, books and matching

| File | Types, methods and responsibility |
| --- | --- |
| [domain/trade/mod.rs](../../../core/src/domain/trade/mod.rs) | Exports allocation, book, caravan, execution, network and offer modules. |
| [domain/trade/offer.rs](../../../core/src/domain/trade/offer.rs) | `TradeOffer`, `BuyTradeOffer`, `SellTradeOffer`, `TradeDemand`. `buy`/`sell` construct offers; `remaining_units` reads open quantity; `calculate_demand` requests stock or gold. `without_progress` strips earlier purchases from templates; `take_share`/`absorb` split/merge offers. Buy/sell accessors track stock and purchases. Buy gold demand uses saturating multiplication. |
| [domain/trade/book.rs](../../../core/src/domain/trade/book.rs) | `TradeBook` owns reserved funds and offers. `new`/`with_offers`/`default` construct it; `replace_offers` installs templates; `calculate_demand` sums reservations; `update_sell_resource_amount` and `add_gold_amount` apply funding. `gold`, `offers`, `goods_for_sale`, `bought_goods` expose state; `return_resources` drains materials and funds. |
| [domain/trade/book/matching.rs](../../../core/src/domain/trade/book/matching.rs) | `TradeBook::trade` returns executions from self's perspective. Private `transaction` scales purchases to gold, orders price/resource ties and spends remainders. `execute_single_trade` updates buyer/seller quantities and funds. |
| [domain/trade/book/market.rs](../../../core/src/domain/trade/book/market.rs) | `TradeBook::trade_with_visitors` shares a market and returns execution vectors in visitor order. `visitor_interest`, `price`, `equal_shares`, `split`, `take_share`, `absorb` implement repeated sharing. |
| [domain/trade/execution.rs](../../../core/src/domain/trade/execution.rs) | `TradeExecution`: resource, units, unit price, `TradeSide`. `total_price` multiplies in `u64`; `mirrored` and `TradeSide::opposite` switch perspective. |
| [domain/trade/allocation.rs](../../../core/src/domain/trade/allocation.rs) | `TradeAllocationEngine::execute` withdraws available stock/gold, reducing unfunded sell quantities; `execute_gold_allocation` handles funds. Legacy `execute_remained_resources` ignores material admission rejection and is not used by the coordinated lifecycle. |

## Settlement state and lifecycle

| File | Types, methods and responsibility |
| --- | --- |
| [domain/settlement/model.rs](../../../core/src/domain/settlement/model.rs) | Owns treasury, funded market book and private `TradeManager`; exports `CaravanOrder` and `TradeCommandError`. |
| [domain/settlement/model/commands/trade.rs](../../../core/src/domain/settlement/model/commands/trade.rs) | Public commands/views on Settlement, listed below. |
| [domain/settlement/model/trading/mod.rs](../../../core/src/domain/settlement/model/trading/mod.rs) | `CaravanOrder`: receiver ID and offers. Private `TradeManager`: templates, pending orders, funded departures and pending returns. |
| [domain/settlement/model/trading/lifecycle.rs](../../../core/src/domain/settlement/model/trading/lifecycle.rs) | `reserve_market` rebuilds/funds the market; `fund_caravan_orders` drains the queue after market funding; `trade_with_visitors` matches eligible arrivals and starts return journeys; `collect_trade_returns` credits gold and combines market/caravan materials for storage. Includes lifecycle unit tests. |
| [domain/settlement/model/turn/mod.rs](../../../core/src/domain/settlement/model/turn/mod.rs) | `prepare_turn(environment, visitors)` reserves/funds/matches before allocation. `finish_turn` merges trade receipts with output before admission/closing snapshots. Public `process_turn(environment, visitors)` runs the same phases locally. Map operations remain behind capabilities. |
| [domain/settlement/model/turn/report.rs](../../../core/src/domain/settlement/model/turn/report.rs) | `TurnReport.trade`, `TradeTurnReport`, `MarketTrade`, `DispatchedCaravan`, `ReturnedCaravan`. `TradeTurnReport::reserved` totals outgoing market/departure materials. `InventoryTurnReport::is_balanced` accounts for reservations/receipts. Report records do not own runtime cargo. |
| [domain/settlement/model/economy/inventory.rs](../../../core/src/domain/settlement/model/economy/inventory.rs) | `process_production_results` admits internal returns and produced/trade goods in storage order. `commit_pending_production` transfers buffered materials; both report rejected quantities. |
| [domain/economy/gold_inventory.rs](../../../core/src/domain/economy/gold_inventory.rs) | Currency counter: `amount`, `add`, `consume`, `consume_available`; no warehouse capacity. |

### Public commands and views

| Method | Contract |
| --- | --- |
| `deposit_gold(amount)`, `gold()` | Seed/read treasury currency. |
| `publish_trade_offers(offers)` | Reject labour, remove previous purchase progress and replace persistent templates. |
| `clear_trade_offers()`, `market_offers()` | Clear/read published templates. |
| `order_caravan(receiver_id, offers)` | Reject self-trade, empty open demand and labour; queue a normalized one-shot order. Registry membership/routes are checked later by World. |
| `cancel_caravan_orders()`, `caravan_orders()` | Clear/read unfunded orders; do not cancel funded or traveling cargo. |
| `trade_book()` | Read funded market state, normally drained between turns. |
| `take_dispatched_caravans()` | Drain funded cargo for routing by the coordinator. |
| `receive_caravan(returned)` | Queue cargo for unloading/reporting at the next local economic update. |

`TradeCommandError` contains `LabourIsNotTradable`, `TradeWithSelf`, `EmptyOrder`.
Agent trade actions (`PublishMarketOffers`, `ClearMarketOffers`, `OrderCaravan`) are
defined in the [decision contract](settlement_decisions.md); they are validated and executed by `World::apply_decision`.

## Routing and global orchestration

| File | Types, methods and responsibility |
| --- | --- |
| [domain/trade/caravan.rs](../../../core/src/domain/trade/caravan.rs) | `CaravanCargo` owns a funded book before routing; `CaravanReturn` owns its returned book/outcome/log. `Caravan::from_cargo` returns cargo on route failure; `new` returns `Option` without recovering its book. `travel` dispatches outbound/return movement; `finish_trading` mirrors executions and starts return; `abort_trading` marks a missing receiver; `into_return` extracts results. `state`, `position`, `book`, `trades` expose views; public `trade` handles one book outside shared matching. |
| [domain/world/state.rs](../../../core/src/domain/world/state.rs) | Owns caravans and settlement/province registries. `caravan_travel` advances movement; internal `settlement_ids` supplies registry order and `settlement_mut` resolves persistent instances. |
| [domain/world/state/trade.rs](../../../core/src/domain/world/state/trade.rs) | `begin_trade_turn` moves/removes caravans, queues returns and turns back missing receivers. `prepare_settlement_turn` supplies sender-sorted arrivals and a map adapter. `default_trade_locations` chooses the first land tile per registered province. `finish_trade_turn` routes departures or queues unchanged cargo on failure. |
| [domain/world/state/construction.rs](../../../core/src/domain/world/state/construction.rs) | `validate_settlement_turn` checks project membership before mutation; `process_settlement_turn` runs one settlement with visitors/environment, without global transport. |
| [domain/world/settlement_environment.rs](../../../core/src/domain/world/settlement_environment.rs) | Temporary construction/productivity adapters; Settlement does not receive Map/PathFinder/TerritoryManager directly. |
| [application/simulation/turn.rs](../../../core/src/application/simulation/turn.rs) | `step` preflights all settlements, applies decisions, starts transport, runs global phases, routes departures and increments time. |
| [domain/trade/network.rs](../../../core/src/domain/trade/network.rs) | Alternative `process_turn(world, locations, finder)`: preflight all, start transport, run local turns by ID and route departures. Explicit endpoints; no Simulation counter. |
| [domain/world/pathfinding/a_star.rs](../../../core/src/domain/world/pathfinding/a_star.rs) | `AStar::a_star` returns point paths; terrain costs affect route choice. |
| [domain/world/pathfinding/path_finder.rs](../../../core/src/domain/world/pathfinding/path_finder.rs) | Reusable search buffers owned/supplied by the coordinator. |

This integration preserves existing matching, movement and temporary endpoint
selection. Use exactly one whole-world runner for an intended turn.

## Failure and report timing

| Situation | Result |
| --- | --- |
| Invalid command | Immediate `TradeCommandError`; existing templates/orders unchanged. |
| Entire order unfunded | Order consumed; receiver recorded in `unfunded_caravan_orders`. |
| Partial funding | Available cargo/funds can leave; unfulfilled quantities are not requeued. |
| Missing receiver at dispatch | Queue unchanged cargo with `ReceiverMissing`. |
| Missing endpoint or A* route | Queue unchanged cargo with `NoRoute`. |
| Missing receiver on arrival | Start return journey; later report `ReceiverMissing`. |
| No compatible trades | Normal return with `Traded` and empty executions. |
| Material capacity exceeded | Storage order decides admission; rejected quantities become inventory losses. |

Routing occurs after local reports are assembled. A failed departure is reported
as funded now and as a return when unloaded next turn. Market executions are seen
from the host; returned caravan logs use the sender's perspective. The material
balance excludes gold and cargo outside warehouse/buffer snapshots.
