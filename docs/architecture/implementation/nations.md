# Nations

A nation is a state agent that owns settlements (gameplay specification §5). It
produces and trades nothing itself. It holds a treasury, a national stock and a
policy, and submits decisions through the same CQRS flow as settlements.

Status: ownership, capital, treasury, stock, extinction, subsidies, the tax-rate
policy, nation decisions, the nation's turn report and nation observations are
implemented. Tax collection, decrees and diplomacy extend this contract.

| Part | Source |
| --- | --- |
| `Nation` (treasury, stock, tax rate, capital) | [domain/nation/mod.rs](../../../core/src/domain/nation/mod.rs) |
| `NationRegistry` (nations and ownership) | [domain/nation/registry.rs](../../../core/src/domain/nation/registry.rs) |
| `NationTurnReport` | [domain/nation/report.rs](../../../core/src/domain/nation/report.rs) |
| Ownership changes, capital succession, extinction, subsidies | [domain/world/state/nations.rs](../../../core/src/domain/world/state/nations.rs) |
| Decision contract and results | [application/commands/nation/contract.rs](../../../core/src/application/commands/nation/contract.rs), [response.rs](../../../core/src/application/commands/nation/response.rs) |
| Validation and execution | [preparation.rs](../../../core/src/application/commands/nation/preparation.rs), [handler.rs](../../../core/src/application/commands/nation/handler.rs) |
| Observation | [queries/nation_observation.rs](../../../core/src/application/queries/nation_observation.rs), [queries/observe_nation.rs](../../../core/src/application/queries/observe_nation.rs) |
| Tests | [core/tests/nation_tests.rs](../../../core/tests/nation_tests.rs) |

## Ownership

**Every settlement belongs to exactly one nation.** `NationRegistry` keeps the
only record `settlement -> nation`; a nation does not store its settlements, so
ownership cannot disagree between two places. A province belongs to the owner of
its settlement; an empty province belongs to nobody (`World::province_owner`).

| Method | Rule |
| --- | --- |
| `add_settlement`, `add_settlement_at`, `initialize_settlement(_at)` | The settlement founds a new nation and becomes its capital. Nation IDs are consecutive and never reused. |
| `add_settlement_to_nation(nation, province, settlement)` | The settlement joins an existing nation (scenario setup). |
| `transfer_settlement(settlement, to)` | Gives a settlement to another nation (later: conquest, §11.5). Fails without changes for unknown IDs or when `to` already owns it. Returns `OwnershipChange`. |

A failed registration changes nothing and founds no nation. After a transfer the
previous owner:

1. **gets a new capital** if it lost its capital: the most populous remaining
   settlement, lowest ID on ties; the capital-loss penalty runs for
   `Nation::CAPITAL_LOSS_PENALTY_TURNS` (10) turns (its effect on happiness
   arrives with the happiness model);
2. **keeps its whole stock**: the capacity shrinks, nothing is discarded, and a
   stock above the capacity accepts nothing until it drops below;
3. **is removed when it owns nothing**: treasury and stock vanish (`gold_lost`,
   `stock_lost`, `extinct`). `Simulation::transfer_settlement` also discards its
   pending decisions and last report.

## Treasury, stock and tax rate

- Treasury: gold, never above `u32::MAX` (`deposit_nation_gold` returns the
  accepted amount).
- Stock: one warehouse (`ResourceInventory`, as in settlements) whose single
  capacity, `Nation::STOCK_CAPACITY_PER_SETTLEMENT` (500) × owned settlements,
  is shared by all materials. The registry updates it on every ownership change.
  Additions above it are lost (`deposit_nation_resource` reports the rejected
  part). Labour is never stored. Gold is separate, in the treasury.
- Tax rate: a plain `f32`, any fraction from `0.0` (0%) to `1.0` (100%),
  starting at `Nation::BASE_TAX_RATE` (0.1). The validator rejects other values
  with `invalid_tax_rate`. Collection is a separate step.

## Decisions

```json
{"nation_id": 0, "actions": [
  {"type": "set_tax_rate", "rate": 0.15},
  {"type": "subsidize", "settlement_id": 2, "gold": 50, "resources": {"Wood": 20}}
]}
```

| Action | Effect | Errors |
| --- | --- | --- |
| `set_tax_rate` | `tax_rate_set` | `invalid_tax_rate` outside 0.0..=1.0, `conflict` for a second rate in the same decision |
| `subsidize` | `subsidy_delivered` with `gold`, `delivered` and `returned` | `unknown_settlement`, `settlement_not_owned`, `empty_subsidy`, `labour_is_not_transferable`, `insufficient_treasury`, `insufficient_stock`, `settlement_treasury_full` |

Actions are checked in order and each one is accepted or rejected on its own; a
rejected action changes and claims nothing. Subsidies of one decision share the
treasury and stock, so `available` in an error is what earlier accepted subsidies
left. Materials that do not fit the settlement's warehouse stay in the national
stock (`returned`): a subsidy neither creates nor destroys goods.

## Global turn

```
nation decisions -> settlement decisions -> Preparation -> Allocation ->
Consumption -> Construction -> Production -> EconomicUpdates -> trade dispatch ->
national timers -> nation reports
```

`Simulation::queue_nation_decision` accepts decisions of existing nations. Nation
decisions run first (ascending nation ID, then submission order), so settlements
decide under the new policy and can use this turn's subsidies.
`SimulationTurnReport::nation_decisions` holds the outcomes and
`SimulationTurnReport::nations` each nation's own `NationTurnReport` (tax rate,
capital, capital-loss turns, settlements, closing treasury and stock). Settlement
decisions stay independent: a nation never builds, staffs or trades for its
settlements.

## Observations

`Simulation::observe_nation(id)` / `observe_all_nations()` return a
`NationObservation`: capital, capital-loss turns, tax rate, treasury, stock,
stored total and shared capacity, a summary of each own settlement (province, population,
gold, free warehouse capacity), the IDs of other nations and `last_turn`. Another
nation's treasury, stock and policy are not visible.

A settlement's `SettlementObservation::nation` shows its owner's policy (nation,
tax rate, capital, whether it is the capital). Each `TradePartner` carries the
partner's `nation_id`.
