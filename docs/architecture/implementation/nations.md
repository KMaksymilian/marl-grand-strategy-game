# Nations

A nation is a state agent that owns settlements (gameplay specification §5). It
produces and trades nothing itself. It holds a treasury, a national stock and a
policy, and submits decisions through the same CQRS flow as settlements.

Status: ownership, capital, treasury, stock, extinction, subsidies, the tax
rate, the production tax, nation decisions, the nation's turn report and nation
observations are implemented. The gold tax waits for settlement income and
happiness (see [For the economy](#for-the-economy-happiness-and-income)).
Decrees are implemented as a catalogue loaded from `decrees.toml` (see
[Decrees](#decrees-7)); their effects are data for the systems that apply them.
Diplomacy extends this contract.

| Part | Source |
| --- | --- |
| `Nation` (treasury, stock, tax rate, capital) | [domain/nation/mod.rs](../../../core/src/domain/nation/mod.rs) |
| `NationRegistry` (nations and ownership) | [domain/nation/registry.rs](../../../core/src/domain/nation/registry.rs) |
| `NationTurnReport` | [domain/nation/report.rs](../../../core/src/domain/nation/report.rs) |
| `Ownership` (owner, conquest turns, under attack) | [domain/nation/registry.rs](../../../core/src/domain/nation/registry.rs) |
| `tax_amount`, `TaxStatus`, tax reports | [domain/nation/taxation.rs](../../../core/src/domain/nation/taxation.rs) |
| Tax collection | [domain/world/state/taxation.rs](../../../core/src/domain/world/state/taxation.rs) |
| Ownership changes, capital succession, extinction, subsidies | [domain/world/state/nations.rs](../../../core/src/domain/world/state/nations.rs) |
| Decision contract and results | [application/commands/nation/contract.rs](../../../core/src/application/commands/nation/contract.rs), [response.rs](../../../core/src/application/commands/nation/response.rs) |
| Validation and execution | [preparation.rs](../../../core/src/application/commands/nation/preparation.rs), [handler.rs](../../../core/src/application/commands/nation/handler.rs) |
| Observation | [queries/nation_observation.rs](../../../core/src/application/queries/nation_observation.rs), [queries/observe_nation.rs](../../../core/src/application/queries/observe_nation.rs) |
| `DecreeId`, `DecreeEffects`, `DecreeState`, `DecreeBook` | [domain/nation/decree.rs](../../../core/src/domain/nation/decree.rs) |
| `DecreeCatalog`, `DecreeRules` (loaded from [decrees.toml](../../../decrees.toml)) | [domain/nation/decree_catalog.rs](../../../core/src/domain/nation/decree_catalog.rs) |
| Decree operations in World | [domain/world/state/decrees.rs](../../../core/src/domain/world/state/decrees.rs) |
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
| Status | `under_attack`: rate 0. Freshly conquered (`Ownership::conquest_turns` above zero): half the rate. The shared conquest timer lasts four turns by default. Otherwise the nation's rate. |
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

The [settlement economy](settlement_happiness.md) computes happiness and
gold income from configured rules. The world reads fiscal and ownership
state from the nation layer each turn.

| What | Where | Rule from the specification |
| --- | --- | --- |
| Tax factor of happiness | `Nation::tax_rate` ([nation/mod.rs](../../../core/src/domain/nation/mod.rs)) | `10 − rate × 100` points in every settlement of the nation |
| Capital-loss penalty | `Nation::capital_loss_turns` ([nation/mod.rs](../../../core/src/domain/nation/mod.rs)) | −10 happiness while above zero (10 turns) |
| Conquest penalty | `Ownership::conquest_turns` ([nation/registry.rs](../../../core/src/domain/nation/registry.rs)) | `−5 × conquest_turns` happiness (−20, −15, −10, −5) |
| War weariness | `Nation::war_weariness` | Subtract the national value from each settlement's score |
| Gold income | Settlement economic updates | Credit income from opening population and the turn's happiness snapshot |

The capital-loss and conquest counters count down at the end of every turn.
The nation observation also exposes the current war weariness value.

## Decrees (§7)

Decrees are content. [decrees.toml](../../../decrees.toml) holds the common
`[rules]` (base upkeep per settlement, minimum active turns, cooldown, slots) and
one `[[decrees]]` entry per decree: `id`, `name`, `cost_multiplier` and
`effects`. `DecreeCatalog::standard()` is the bundled file; `DecreeCatalog::load`
reads one from disk and `World::set_decree_catalog` installs it (the game does
this at startup; refused with `DecreeCatalogInUse` once a decree has a state).
Loading rejects unknown fields, invalid or duplicate IDs, empty names, a zero
multiplier, production tiers outside 1–3, non-finite numbers and contradictory
rules.

| Rule | Value in the bundled file |
| --- | --- |
| Upkeep per active turn | `10 × owned settlements × cost_multiplier`, from the treasury, paid right after nation decisions |
| Pending turn | the turn of enactment: no effect, no upkeep; enacting costs nothing |
| Minimum duration | 5 active turns before `revoke_decree` |
| Cooldown | 5 turns after a revocation |
| Unpaid upkeep | revoked exactly like a revocation (cooldown), listed in `decree_upkeep.revoked_unpaid` |
| Slots | 2; pending and active decrees use one each (more from happiness 70, see below) |

Effects are data (`DecreeEffects`), never applied by the nation layer. In the
file a missing field is neutral (fraction 0, multiplier 1, flag false); across
decrees fractions add up, multipliers multiply, flags combine with "or".
Production changes name `categories` and `tiers` of `resources.toml`
(`{ categories = ["raw_food"], tiers = [1], change = 0.2 }`; an empty filter
matches every material), so `DecreeEffects::production_change(definition)`
gives the total change for one material. `World::decree_effects(settlement)`
returns the owner's combined effects; places for the economy are marked
`ECONOMY` (production, construction cost, caravan speed, happiness, monument
multiplier, food consumption, slots from average happiness). Tariffs, the
military levy, army and walls fields wait for their systems.

## Decisions

```json
{"nation_id": 0, "actions": [
  {"type": "set_tax_rate", "rate": 0.15},
  {"type": "subsidize", "settlement_id": 2, "gold": 50, "resources": {"Wood": 20}},
  {"type": "enact_decree", "decree": "agricultural_decree"},
  {"type": "revoke_decree", "decree": "games"}
]}
```

| Action | Effect | Errors |
| --- | --- | --- |
| `set_tax_rate` | `tax_rate_set` | `invalid_tax_rate` outside 0.0..=1.0, `conflict` for a second rate in the same decision |
| `subsidize` | `subsidy_delivered` with `gold`, `delivered` and `returned` | `unknown_settlement`, `settlement_not_owned`, `empty_subsidy`, `labour_is_not_transferable`, `insufficient_treasury`, `insufficient_stock`, `settlement_treasury_full` |
| `enact_decree` | `decree_enacted` with the `upkeep` per active turn | `unknown_decree`, `decree_already_enacted`, `decree_on_cooldown` (`turns_left`), `no_free_decree_slot`, `conflict` for the same decree twice |
| `revoke_decree` | `decree_revoked` with `cooldown_turns` | `unknown_decree`, `decree_not_active` (also a pending one), `decree_minimum_duration` (`turns_left`), `conflict` |

A decree ID only needs the snake-case shape to parse; whether it exists in the
catalogue is checked by validation (`unknown_decree`), so the contract does not
change when the file does. Decrees of one decision share the slots: a
revocation frees its slot for a later enactment of the same decision.

Actions are checked in order and each one is accepted or rejected on its own; a
rejected action changes and claims nothing. Subsidies of one decision share the
treasury and stock, so `available` in an error is what earlier accepted subsidies
left. Materials that do not fit the settlement's warehouse stay in the national
stock (`returned`): a subsidy neither creates nor destroys goods.

## Global turn

```
nation decisions -> decree upkeep -> settlement decisions -> Preparation -> Allocation ->
Consumption -> Construction -> Production -> Taxation -> EconomicUpdates ->
trade dispatch -> national timers -> nation reports
```

`Simulation::queue_nation_decision` accepts decisions of existing nations. Nation
decisions run first (ascending nation ID, then submission order), so settlements
decide under the new policy and can use this turn's subsidies.
`SimulationTurnReport::nation_decisions` holds the outcomes and
`SimulationTurnReport::nations` each nation's own `NationTurnReport` (tax rate,
capital, capital-loss turns, settlements, closing treasury and stock, taxes,
decree states and `decree_upkeep`). Decrees advance with the national timers:
pending becomes active, active counts a turn, cooldowns run down. Settlement
decisions stay independent: a nation never builds, staffs or trades for its
settlements.

## Observations

`Simulation::observe_nation(id)` / `observe_all_nations()` return a
`NationObservation`: capital, capital-loss turns, tax rate, treasury, stock,
stored total and shared capacity, a summary of each own settlement (province,
population, gold, conquest turns, under attack, free warehouse capacity), the IDs of other nations,
`decrees` (slots, used slots, minimum duration, cooldown, states, the upkeep of
every decree of the catalogue and the combined effects) and `last_turn`. Another
nation's treasury, stock and policy are not visible.

A settlement's `SettlementObservation::nation` shows its owner's policy (nation,
tax rate, capital, whether it is the capital, active decrees). Each `TradePartner` carries the
partner's `nation_id`.
