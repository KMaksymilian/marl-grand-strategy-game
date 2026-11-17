# Nations

A nation is a state agent that owns settlements (gameplay specification §5). It
produces and trades nothing itself. It holds a treasury, a national stock and a
policy, and it submits decisions through the same CQRS flow as settlements.

Status: ownership, capital, treasury, stock, extinction, subsidies, the tax-rate
policy, nation decisions and nation observations are implemented. Tax collection,
decrees, diplomacy and armies are planned as separate tasks and will extend the
same contract.

| Part | Source |
| --- | --- |
| `Nation`, `NationalStock`, `TaxRate` | [domain/nation/mod.rs](../../../core/src/domain/nation/mod.rs) |
| `NationRegistry` (nations and ownership) | [domain/nation/registry.rs](../../../core/src/domain/nation/registry.rs) |
| Ownership changes, capital succession, extinction, transfers | [domain/world/state/nations.rs](../../../core/src/domain/world/state/nations.rs) |
| Decision contract and results | [application/commands/nation/contract.rs](../../../core/src/application/commands/nation/contract.rs), [response.rs](../../../core/src/application/commands/nation/response.rs) |
| Validation and execution | [preparation.rs](../../../core/src/application/commands/nation/preparation.rs), [handler.rs](../../../core/src/application/commands/nation/handler.rs) |
| Observation | [queries/nation_observation.rs](../../../core/src/application/queries/nation_observation.rs), [queries/observe_nation.rs](../../../core/src/application/queries/observe_nation.rs) |
| Tests | [core/tests/nation_tests.rs](../../../core/tests/nation_tests.rs) |

## Ownership

`NationRegistry` keeps the only record `settlement -> nation`. A nation does not
store its settlements, so ownership cannot disagree between two places. A
settlement without an entry is independent. A province belongs to the owner of its
settlement; an empty province belongs to nobody (`World::province_owner`).

Only `World` changes ownership:

| Method | Rule |
| --- | --- |
| `found_nation(capital)` | An existing, independent settlement founds a nation and becomes its capital. IDs are consecutive and never reused. |
| `transfer_settlement(settlement, to)` | Gives an independent or owned settlement to an existing nation. Fails without changes for unknown IDs or when `to` already owns it. Returns `OwnershipChange`. |

After a transfer, the previous owner:

1. **Gets a new capital** if it lost its capital: the most populous remaining
   settlement, lowest ID on ties. The capital-loss penalty runs for
   `Nation::CAPITAL_LOSS_PENALTY_TURNS` (10) turns and counts down at the end of
   every global turn (`capital_loss_turns`). Its effect on happiness belongs to
   the happiness model (§4.2).
2. **Loses stock above its reduced capacity**, reported in `stock_lost`.
3. **Is removed when it owns nothing.** Its treasury and stock vanish and are
   reported in `gold_lost` and `stock_lost`; `extinct` is set.
   `Simulation::transfer_settlement` also discards its pending decisions.

## Treasury and national stock

- Treasury: gold, never above `u32::MAX` (`deposit_nation_gold` returns the
  accepted amount).
- Stock: each material has its own capacity,
  `Nation::STOCK_CAPACITY_PER_SETTLEMENT` (500) × owned settlements. Additions
  above it are lost (`deposit_nation_resource` reports the rejected part). Labour
  is never stored.
- `deposit_nation_gold` and `deposit_nation_resource` are the entry points for
  scenarios and, later, for taxes and customs.

## Policy

`TaxRate` is the common rate of all own settlements in basis points
(1 = 0.01%), `0..=10000`, starting at `TaxRate::BASE` (1000 = 10%). It serializes
as a plain integer; values above 10000 do not parse. Collection is part of the
taxation task.

## Decisions

```json
{"nation_id": 0, "actions": [
  {"type": "set_tax_rate", "rate": 1500},
  {"type": "subsidize", "settlement_id": 2, "gold": 50, "resources": {"Wood": 20}}
]}
```

| Action | Effect | Errors |
| --- | --- | --- |
| `set_tax_rate` | `tax_rate_set` | `conflict` for a second rate in the same decision |
| `subsidize` | `subsidy_delivered` with `gold`, `delivered` and `returned` | `unknown_settlement`, `settlement_not_owned`, `empty_subsidy`, `labour_is_not_transferable`, `insufficient_treasury`, `insufficient_stock`, `settlement_treasury_full` |

The policy matches settlement decisions: actions are checked in order and each
one is accepted or rejected on its own; a rejected action changes and claims
nothing. Subsidies of one decision share the treasury and stock, so `available`
in an error is what earlier accepted subsidies left. Materials that do not fit
the settlement's warehouse stay in the national stock (`returned`), so a subsidy
neither creates nor destroys goods.

Missing `actions` means none; missing `gold` or `resources` mean zero. Unknown
fields and tags, negative or fractional amounts and rates above 10000 fail to
parse with `invalid nation decision`.

## Global turn

`Simulation::queue_nation_decision` accepts decisions of existing nations. In
`step`, nation decisions run first, in ascending nation ID and submission order,
before settlement decisions. Settlements therefore decide under the new policy
and can use this turn's subsidies. `SimulationTurnReport::nation_decisions` holds
one `NationDecisionOutcome` per decision. National timers count down after the
economic phases.

Settlement decisions stay independent: a nation never builds, staffs or trades
for its settlements.

## Observations

`Simulation::observe_nation(id)` and `observe_all_nations()` return a
`NationObservation`: capital, capital-loss turns, tax rate, treasury, stock,
per-material capacity, a summary of each own settlement (province, population,
gold, free warehouse capacity) and the IDs of other nations. Another nation's
treasury, stock and policy are not visible.

A settlement's `SettlementObservation::nation` shows its owner's policy (nation,
tax rate, capital, whether it is the capital); it is `null` for an independent
settlement. Each `TradePartner` carries the partner's `nation_id`.
