# Nations

A nation is a state agent that owns settlements (gameplay specification §5). It
produces and trades nothing itself. It holds a treasury, a national stock and a
policy, and submits decisions through the same CQRS flow as settlements.

Status: ownership, capital, treasury, stock, extinction, subsidies, the tax
rate, the production tax, decrees with capital relocation, nation decisions,
the nation's turn report and nation observations are implemented. The gold tax
and the decree effects on the settlement economy wait for the economy (see
[For the economy](#for-the-economy-happiness-and-income)). Diplomacy extends
this contract.

| Part | Source |
| --- | --- |
| `Nation` (treasury, stock, tax rate, capital) | [domain/nation/mod.rs](../../../core/src/domain/nation/mod.rs) |
| `NationRegistry` (nations and ownership) | [domain/nation/registry.rs](../../../core/src/domain/nation/registry.rs) |
| `NationTurnReport` | [domain/nation/report.rs](../../../core/src/domain/nation/report.rs) |
| `Ownership` (owner, conquest turns, under attack) | [domain/nation/registry.rs](../../../core/src/domain/nation/registry.rs) |
| `tax_amount`, `TaxStatus`, tax reports | [domain/nation/taxation.rs](../../../core/src/domain/nation/taxation.rs) |
| Tax collection | [domain/world/state/taxation.rs](../../../core/src/domain/world/state/taxation.rs) |
| `Decree`, `DecreeEffects`, `DecreeBook`, relocation types | [domain/nation/decree.rs](../../../core/src/domain/nation/decree.rs) |
| Decree lifecycle, effects lookup, relocation | [domain/world/state/decrees.rs](../../../core/src/domain/world/state/decrees.rs) |
| Ownership changes, capital succession, extinction, subsidies | [domain/world/state/nations.rs](../../../core/src/domain/world/state/nations.rs) |
| Decision contract and results | [application/commands/nation/contract.rs](../../../core/src/application/commands/nation/contract.rs), [response.rs](../../../core/src/application/commands/nation/response.rs) |
| Validation and execution | [preparation.rs](../../../core/src/application/commands/nation/preparation.rs), [handler.rs](../../../core/src/application/commands/nation/handler.rs) |
| Observation | [queries/nation_observation.rs](../../../core/src/application/queries/nation_observation.rs), [queries/observe_nation.rs](../../../core/src/application/queries/observe_nation.rs) |
| Tests | [nation_tests.rs](../../../core/tests/nation_tests.rs), [taxation_tests.rs](../../../core/tests/taxation_tests.rs), [decree_tests.rs](../../../core/tests/decree_tests.rs) |

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

## Taxation (§6)

The nation goes through its settlements and takes its rate from what each one
produced this turn: `tax = rate × produced`, summed into the national stock.

It happens in the `Taxation` phase of every global turn, after `Production` and
before `EconomicUpdates`, for every settlement in ascending ID.

| Step | Rule |
| --- | --- |
| Status | `under_attack`: rate 0. Freshly conquered (`Ownership::conquest_turns` above zero, 4 turns after a transfer): half the rate. Otherwise the nation's rate. |
| Production tax | Every material of this turn's gross production, separately, rounded down (`tax_amount`). Taken before the buffer and warehouse, so storage losses never reduce it. Intermediate goods are taxed at every stage. Labour is not taxed. |
| National side | Materials go to the stock up to its shared capacity; what does not fit is lost and reported. Settlements of one nation fill the stock in ascending ID. |
| Gold | Not taxed yet (see below). |

`World::set_settlement_under_attack` marks a siege until the war system sets it
from army positions. A settlement's `InventoryTurnReport::taxed` lists its tax,
and `is_balanced` counts it as an outflow.

Reports: `TurnReport::taxation` (each settlement's `SettlementTaxReport`: nation,
status, nominal and effective rate, production tax) and `NationTurnReport::taxes`
(`NationTaxReport`: received, lost, every own settlement's report).

## For the economy: happiness and income

Settlement happiness (§4.2) and gold income (§4.4) belong to the settlement
economy. The nation layer only provides what they need; places are marked
`ECONOMY` in the code.

| What | Where | Rule from the specification |
| --- | --- | --- |
| Tax factor of happiness | `Nation::tax_rate` ([nation/mod.rs](../../../core/src/domain/nation/mod.rs)) | `10 − rate × 100` points in every settlement of the nation |
| Capital-loss penalty | `Nation::capital_loss_turns` ([nation/mod.rs](../../../core/src/domain/nation/mod.rs)) | −10 happiness while above zero (10 turns) |
| Conquest penalty | `Ownership::conquest_turns` ([nation/registry.rs](../../../core/src/domain/nation/registry.rs)) | `−5 × conquest_turns` happiness (−20, −15, −10, −5) |
| Gold tax | `World::collect_taxes` ([world/state/taxation.rs](../../../core/src/domain/world/state/taxation.rs)) | take `income × effective_rate`, deposit with `Nation::deposit_gold` |
| Gold tax in reports | `SettlementTaxReport` ([nation/taxation.rs](../../../core/src/domain/nation/taxation.rs)) | add `gold_income` and `gold_tax` |
| Decree slots | `World::decree_slots` ([world/state/decrees.rs](../../../core/src/domain/world/state/decrees.rs)) | pass the average happiness of the nation's settlements to `DecreeBook::slots` |
| Decree effects | `World::decree_effects(settlement)` ([world/state/decrees.rs](../../../core/src/domain/world/state/decrees.rs)) | apply the `DecreeEffects` fields marked `ECONOMY` |
| Production scopes | `ProductionScope` ([nation/decree.rs](../../../core/src/domain/nation/decree.rs)) | map each scope to resources of the catalogue (tier, group) |

`DecreeEffects` fields for the economy: `production` (fraction per scope),
`construction_cost`, `caravan_speed`, `happiness` (points),
`monument_happiness_multiplier`, `food_consumption`.

All three penalty counters count down at the end of every turn; reading them is
enough. Nation and settlement observations already show them.

## Decrees (§7)

| Decree (`decree`) | Advantage | Drawback | Cost × |
| --- | --- | --- | --- |
| Dekret rolny (`agricultural_decree`) | +20% raw food | −10% raw building materials | 1 |
| Reforma rzemiosła (`craft_reform`) | +20% tier 2 and 3 | −10% tier 1 | 1 |
| Górnictwo państwowe (`state_mining`) | +30% ores | −5 happiness | 1 |
| Wielka budowa (`great_construction`) | −20% construction materials | | 2 |
| Wolny handel (`free_trade`) | tariffs ×0.5, caravans +25% faster | less customs income | 1 |
| Protekcjonizm (`protectionism`) | tariffs ×2 | | 1 |
| Kontyngent wojskowy (`military_levy`) | production tax on planks, steel, bronze, cattle ×2 | −5 happiness | 1 |
| Mobilizacja (`mobilization`) | conscription pool ×2, instant recruitment | −15% production, −10 happiness | 1 |
| Dyscyplina (`discipline`) | morale losses ×0.5 | army upkeep ×1.25 | 1 |
| Racjonowanie jedzenia (`food_rationing`) | −25% food consumption | −15 happiness | 1 |
| Igrzyska (`games`) | +15 happiness | | 4 |
| Mecenat sztuki (`art_patronage`) | monument happiness ×2 | | 3 |
| Fortyfikacja kraju (`national_fortification`) | wall bonus +10 points, walls −30% cost | | 4 |

| Rule | Value |
| --- | --- |
| Cost | **One-time**, paid when enacted: `10 × owned settlements × multiplier` gold. Not enough gold: `insufficient_treasury`. Revoking refunds nothing. (The specification's "per turn" was replaced by a one-time payment.) |
| Slots | 2. `DecreeBook::slots` adds one at average settlement happiness 70 and one per further 10 points (6 at 100); without happiness every nation has 2. Pending and active decrees use slots; cooling-down ones do not. |
| Activation | Enacted in turn T (`pending`), in effect from turn T+1 (`active`). |
| Minimum duration | Revocable after 5 active turns; a pending decree cannot be revoked. |
| Cooldown | 5 turns after revocation before the same decree can be enacted again. |

**Effects are data.** `DecreeEffects` combines the owner's active decrees
(fractions add up, multipliers multiply, flags combine) and
`World::decree_effects(settlement)` returns them. The nation layer applies none
of them itself; every field names the system that applies it: the economy
(production, construction cost, caravan speed, happiness, monuments, food
consumption), customs (tariffs), taxation (the military levy, once its resources
exist), the army and walls.

**Capital relocation** is a one-shot decree without a slot. It costs
`100 × owned settlements`, paid when ordered. The target must be an own
settlement that is not the capital and not under attack, and only one relocation
may be pending. At the start of the next turn the capital moves if the target is
still owned and not under attack (no capital-loss penalty); otherwise the order
expires and the cost is refunded (`not_owned`, `under_attack`).

## Decisions

```json
{"nation_id": 0, "actions": [
  {"type": "set_tax_rate", "rate": 0.15},
  {"type": "subsidize", "settlement_id": 2, "gold": 50, "resources": {"Wood": 20}},
  {"type": "enact_decree", "decree": "agricultural_decree"},
  {"type": "revoke_decree", "decree": "games"},
  {"type": "relocate_capital", "settlement_id": 3}
]}
```

| Action | Effect | Errors |
| --- | --- | --- |
| `set_tax_rate` | `tax_rate_set` | `invalid_tax_rate` outside 0.0..=1.0, `conflict` for a second rate in the same decision |
| `subsidize` | `subsidy_delivered` with `gold`, `delivered` and `returned` | `unknown_settlement`, `settlement_not_owned`, `empty_subsidy`, `labour_is_not_transferable`, `insufficient_treasury`, `insufficient_stock`, `settlement_treasury_full` |
| `enact_decree` | `decree_enacted` with the `cost` paid | `decree_already_enacted`, `decree_on_cooldown`, `no_free_decree_slot`, `insufficient_treasury`, `conflict` |
| `revoke_decree` | `decree_revoked` with `cooldown_turns` | `decree_not_active`, `decree_minimum_duration`, `conflict` |
| `relocate_capital` | `capital_relocation_ordered` with the `cost` paid | `unknown_settlement`, `settlement_not_owned`, `already_capital`, `settlement_under_attack`, `relocation_pending`, `insufficient_treasury`, `conflict` |

Actions are checked in order and each one is accepted or rejected on its own; a
rejected action changes and claims nothing. Subsidies, decree costs and a
relocation of one decision share the treasury, and decrees share the slots (a
revocation frees its slot for a later enactment), so `available` in an error is
what earlier accepted actions left. One decision sets the rate once, touches
each decree once and orders at most one relocation. Materials that do not fit the settlement's warehouse stay in the national
stock (`returned`): a subsidy neither creates nor destroys goods.

## Global turn

```
capital relocations -> nation decisions -> settlement decisions -> Preparation -> Allocation ->
Consumption -> Construction -> Production -> Taxation -> EconomicUpdates ->
trade dispatch -> national timers and decree states -> nation reports
```

`Simulation::queue_nation_decision` accepts decisions of existing nations. Nation
decisions run first (ascending nation ID, then submission order), so settlements
decide under the new policy and can use this turn's subsidies.
`SimulationTurnReport::nation_decisions` holds the outcomes and
`SimulationTurnReport::nations` each nation's own `NationTurnReport` (tax rate,
capital, capital-loss turns, settlements, closing treasury and stock, taxes,
decree states, the capital relocation resolved this turn). Settlement
decisions stay independent: a nation never builds, staffs or trades for its
settlements.

## Observations

`Simulation::observe_nation(id)` / `observe_all_nations()` return a
`NationObservation`: capital, capital-loss turns, tax rate, treasury, stock,
stored total and shared capacity, a summary of each own settlement (province,
population, gold, conquest turns, under attack, free warehouse capacity), the IDs
of other nations, `decrees` (slots, used slots, states, the one-time cost of
every decree, relocation cost, pending relocation, combined effects) and
`last_turn`. Another
nation's treasury, stock and policy are not visible.

A settlement's `SettlementObservation::nation` shows its owner's policy (nation,
tax rate, capital, whether it is the capital, active decrees). Each `TradePartner` carries the
partner's `nation_id`.
