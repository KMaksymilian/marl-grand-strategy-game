# Nations

A nation is a state agent that owns settlements (gameplay specification §5). It
produces and trades nothing itself. It holds a treasury, a national stock and a
policy, collects taxes, and submits decisions through the same CQRS flow as
settlements.

Status: ownership, capital, treasury, stock, extinction, subsidies, the tax rate,
the production tax, decrees, capital relocation, nation decisions, nation reports
and nation observations are implemented. The gold tax waits for settlement income
and happiness. Decrees, diplomacy and armies
are separate tasks and extend the same contract.

| Part | Source |
| --- | --- |
| `Nation`, `NationalStock` | [domain/nation/mod.rs](../../../core/src/domain/nation/mod.rs) |
| `TaxRate` | [domain/nation/tax.rs](../../../core/src/domain/nation/tax.rs) |
| `Decree`, `DecreeEffects`, `DecreeBook`, relocation types | [domain/nation/decree.rs](../../../core/src/domain/nation/decree.rs) |
| Decree lifecycle, upkeep, relocation, effect lookups | [domain/world/state/decrees.rs](../../../core/src/domain/world/state/decrees.rs) |
| `TaxStatus`, `SettlementTaxReport`, `NationTurnReport` | [domain/nation/taxation.rs](../../../core/src/domain/nation/taxation.rs) |
| `NationRegistry`, `Ownership` | [domain/nation/registry.rs](../../../core/src/domain/nation/registry.rs) |
| Ownership changes, capital succession, extinction, subsidies | [domain/world/state/nations.rs](../../../core/src/domain/world/state/nations.rs) |
| Tax collection | [domain/world/state/taxation.rs](../../../core/src/domain/world/state/taxation.rs) |
| Decision contract and results | [application/commands/nation/contract.rs](../../../core/src/application/commands/nation/contract.rs), [response.rs](../../../core/src/application/commands/nation/response.rs) |
| Validation and execution | [preparation.rs](../../../core/src/application/commands/nation/preparation.rs), [handler.rs](../../../core/src/application/commands/nation/handler.rs) |
| Observation | [queries/nation_observation.rs](../../../core/src/application/queries/nation_observation.rs), [queries/observe_nation.rs](../../../core/src/application/queries/observe_nation.rs) |
| Tests | [nation_tests.rs](../../../core/tests/nation_tests.rs), [taxation_tests.rs](../../../core/tests/taxation_tests.rs), [decree_tests.rs](../../../core/tests/decree_tests.rs) |

## Ownership

**Every settlement belongs to exactly one nation.** `NationRegistry` keeps the
only record `settlement -> Ownership`. Each `Ownership` holds the nation, the
remaining conquest penalty and whether the settlement is under attack. A nation
does not store its settlements, so ownership cannot disagree between two places.
A province belongs to the owner of its settlement; an empty province belongs to
nobody (`World::province_owner`).

Nations come into existence when settlements register:

| Method | Rule |
| --- | --- |
| `add_settlement`, `add_settlement_at`, `initialize_settlement(_at)` | The settlement founds a new nation and becomes its capital. Nation IDs are consecutive and never reused. |
| `add_settlement_to_nation(nation, province, settlement)` | The settlement joins an existing nation (scenario setup), without a penalty. |
| `transfer_settlement(settlement, to)` | Ownership change after a conquest (§11.5). Starts the conquest penalty and ends an attack. Fails without changes for unknown IDs or when `to` already owns it. Returns `OwnershipChange`. |

A failed registration changes nothing and founds no nation.

After a transfer, the previous owner:

1. **Gets a new capital** if it lost its capital: the most populous remaining
   settlement, lowest ID on ties. The capital-loss penalty runs for
   `Nation::CAPITAL_LOSS_PENALTY_TURNS` (10) turns; its happiness effect (§4.2)
   arrives with the happiness model.
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

## Tax rate

`TaxRate` is the common rate of all own settlements as a fraction: `0.0` is 0%,
`0.1` is 10% (`TaxRate::BASE`, the starting rate), `1.0` is 100%. It serializes as
a plain number; values outside `0..=1`, NaN and infinities do not parse, so an
invalid rate cannot exist. `apply(amount)` rounds down to whole units and never
returns more than `amount`; a relative tolerance of 1e-9 removes floating-point
artifacts (29% of 100 is 29, not 28).

## Taxation (§6)

Taxes are collected in the `Taxation` phase of every global turn, after
`Production` and before `EconomicUpdates`, for every settlement in ascending ID.
The nation simply goes through its settlements and takes its rate from what each
one produced this turn.

| Step | Rule |
| --- | --- |
| Effective rate | `under_attack`: 0. Freshly conquered (penalty above zero): half the rate. Otherwise the nation's rate. |
| Production tax | Every material of this turn's gross production, separately, rounded down. Taken before the buffer and warehouse, so storage losses never reduce it. Intermediate goods are taxed at every stage. |
| National side | Materials go to the stock up to its per-material capacity; what does not fit is lost and reported. Settlements of one nation fill the stock in ascending ID. |

**Gold is not taxed yet.** The gold tax applies to the settlement's gold income
(§4.4), which depends on settlement happiness (§4.2). Neither exists in the
settlement economy yet; the gold tax is added together with them. Until then the
treasury changes only through subsidies and scenario deposits.

The conquest penalty starts at 20 and drops by 5 after every turn: a conquered
settlement pays half for four turns. `World::set_settlement_under_attack` marks a
siege until the war system sets it from army positions.

Material balance: a settlement's `InventoryTurnReport` lists the tax in `taxed`,
and `is_balanced` counts it as an outflow.

## Decrees (§7)

| Decree (`decree`) | Advantage | Drawback | Cost × |
| --- | --- | --- | --- |
| Dekret rolny (`agricultural_decree`) | +20% raw food | −10% raw building materials | 1 |
| Reforma rzemiosła (`craft_reform`) | +20% tier 2 and 3 | −10% tier 1 | 1 |
| Górnictwo państwowe (`state_mining`) | +30% ores | −5 happiness | 1 |
| Wielka budowa (`great_construction`) | −20% construction materials | | 2 |
| Wolny handel (`free_trade`) | tariffs ×0.5, caravans +25% faster | less customs income | 1 |
| Protekcjonizm (`protectionism`) | tariffs ×2 | | 1 |
| Kontyngent wojskowy (`military_levy`) | tax on planks, steel, bronze, cattle ×2 | −5 happiness | 1 |
| Mobilizacja (`mobilization`) | conscription pool ×2, instant recruitment | −15% production, −10 happiness | 1 |
| Dyscyplina (`discipline`) | morale losses ×0.5 | army upkeep ×1.25 | 1 |
| Racjonowanie jedzenia (`food_rationing`) | −25% food consumption | −15 happiness | 1 |
| Igrzyska (`games`) | +15 happiness | | 4 |
| Mecenat sztuki (`art_patronage`) | monument happiness ×2 | | 3 |
| Fortyfikacja kraju (`national_fortification`) | wall bonus +10 points, walls −30% cost | | 4 |

**Lifecycle.**

| Rule | Value |
| --- | --- |
| Slots | 2. `DecreeBook::slots` adds one at average settlement happiness 70 and one per further 10 points (6 at 100); happiness does not exist yet, so every nation has 2. Pending and active decrees use slots; cooling-down decrees do not. |
| Activation | Enacted in turn T (`pending`), in effect from turn T+1 (`active`). |
| Upkeep | `10 × owned settlements × multiplier` gold every turn the decree is active, paid right after nation decisions. A decree the treasury cannot pay **lapses**: it ends, has no effect that turn and starts its cooldown. |
| Minimum duration | Revocable after 5 active turns (`turns_active ≥ 5`); a pending decree cannot be revoked. |
| Cooldown | 5 turns after revocation or lapse before the same decree can be enacted again. |

**Effects** are combined into `DecreeEffects` (fractions add up, multipliers
multiply) and read by the owning systems:

| Effect | System | Status |
| --- | --- | --- |
| Production per resource group | `Settlement::produce_turn` scales each produced resource by `1 + change` (rounded down) before taxation | applied |
| Construction material cost | the settlement validator takes `World::construction_cost` (catalog cost × `1 + change`, rounded up) | applied |
| Caravan speed | `Caravan::change_speed` at dispatch (10 path steps, +25% = 12) | applied |
| Tariffs, happiness, monuments, food consumption, military levy, conscription, recruitment, morale, army upkeep, walls | read by diplomacy, happiness, population, army and wall systems | data only until those systems exist |

Resource groups (§2) come from `ResourceType::tier`, `category` and `is_ore`:
Grain is raw food, Wood and Stone raw building materials; no ore exists yet. The
production change scales building output; for extraction (no inputs) this equals
more production cycles.

**Capital relocation** is a one-shot decree without a slot. It costs
`100 × owned settlements`, paid when ordered. The target must be an own
settlement that is not the capital and not under attack, and only one relocation
may be pending. At the start of the next turn the capital moves if the target is
still owned and not under attack (no capital-loss penalty); otherwise the order
expires and the cost is refunded (`not_owned`, `under_attack`).

## Reports

- `TurnReport::taxation` (each settlement): `SettlementTaxReport` with nation,
  status, nominal and effective rate and production tax.
- `SimulationTurnReport::nations` (each nation, its own report):
  `NationTurnReport` with tax rate, the capital relocation resolved this turn,
  decrees (active, upkeep paid, lapsed), materials received and lost, every own
  settlement's `SettlementTaxReport`, closing treasury and stock.

## Decisions

```json
{"nation_id": 0, "actions": [
  {"type": "set_tax_rate", "rate": 0.15},
  {"type": "subsidize", "settlement_id": 2, "gold": 50, "resources": {"Wood": 20}}
]}
```

| Action | Effect | Errors |
| --- | --- | --- |
| `set_tax_rate` | `tax_rate_set` | `conflict` for a second rate in the same decision |
| `subsidize` | `subsidy_delivered` with `gold`, `delivered` and `returned` | `unknown_settlement`, `settlement_not_owned`, `empty_subsidy`, `labour_is_not_transferable`, `insufficient_treasury`, `insufficient_stock`, `settlement_treasury_full` |
| `enact_decree` | `decree_enacted` with the current `upkeep` | `decree_already_enacted`, `decree_on_cooldown`, `no_free_decree_slot`, `conflict` |
| `revoke_decree` | `decree_revoked` with `cooldown_turns` | `decree_not_active`, `decree_minimum_duration`, `conflict` |
| `relocate_capital` | `capital_relocation_ordered` with `cost` | `unknown_settlement`, `settlement_not_owned`, `already_capital`, `settlement_under_attack`, `relocation_pending`, `insufficient_treasury`, `conflict` |

Actions are checked in order and each one is accepted or rejected on its own; a
rejected action changes and claims nothing. Subsidies and a relocation of one
decision share the treasury and stock; decrees share the slots (a revocation
frees its slot for a later enactment of the same decision). One decision sets the
rate once, touches each decree once and orders at most one relocation. Materials that do not fit the settlement's warehouse stay in
the national stock (`returned`), so a subsidy neither creates nor destroys goods.

## Global turn

```
capital relocations -> nation decisions -> decree upkeep -> settlement decisions ->
Preparation -> Allocation -> Consumption -> Construction -> Production (decree
production changes) -> Taxation -> EconomicUpdates -> trade dispatch (caravan
speed) -> national timers (capital loss, conquest penalty, decree lifecycle)
```

A new rate applies to the same turn's taxes, because nation decisions run first.

## Observations

`Simulation::observe_nation(id)` / `observe_all_nations()` return a
`NationObservation`: capital, capital-loss turns, tax rate, treasury, stock,
per-material capacity, a summary of each own settlement (province, population,
gold, conquest penalty, under attack, free warehouse capacity), the IDs of other
nations, `decrees` (slots, used slots, states, current upkeep of every decree,
relocation cost, pending relocation, combined effects) and `last_turn`, the
nation's latest `NationTurnReport`. Another nation's
treasury, stock and policy are not visible.

A settlement's `SettlementObservation::nation` shows its owner's policy (nation,
tax rate, capital, whether it is the capital, active decrees). Its `last_turn` includes the
production tax. Each `TradePartner` carries
the partner's `nation_id`.
