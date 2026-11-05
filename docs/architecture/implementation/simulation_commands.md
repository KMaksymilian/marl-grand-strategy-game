# Simulation Commands and Reports

The [global lifecycle](../overview/turn_lifecycle.md) defines phase order and the
[registry system](../systems/settlement_registry.md) defines identity/registration.
This reference describes the implemented coordinator API.

## Public API

| Method | Contract |
| --- | --- |
| `Simulation::new(world)` | Take ownership, validate map shape/terrain/province IDs, create search buffers and start at turn zero; return `SimulationError` on validation failure. |
| `world()` | Borrow the authoritative world immutably. |
| `current_turn()` | Read the number of completed global turns. |
| `settlement_registry()`, `settlements()` | Read the single ordered registry/its instances. |
| `settlement(SettlementId)` | Return a shared instance or `WorldError::MissingSettlement`. |
| `add_settlement(province_id, settlement)` | Register an explicitly initialized empty-structure settlement. |
| `initialize_settlement(province_id, capacity, population, needs)` | Allocate an unused ID, create a settlement and register it; return the ID. |
| `queue_decision(decision)` | Queue a `SettlementDecision` of a known settlement; no immediate world mutation. |
| `step()` | Execute one global turn and return `SimulationTurnReport`. |
| `into_world()` | Consume the coordinator and recover its world; pending decisions are discarded. |

The registry retains its ID-allocation cursor in the recovered world. Creating a
new coordinator resets global time to zero; this is not a resume/checkpoint API.
No public mutable world getter allows callers to bypass coordinated mutation.

## Decision contracts

Agents submit a serializable [`SettlementDecision`](settlement_decisions.md):
a settlement ID and a list of `SettlementAction`s (construct, upgrade, cancel,
set workers, storage order, publish/clear market offers, order caravan). Agents
never supply costs, definitions or project IDs; the simulation takes them from
`World::building_catalog()` and `Settlement::next_project_id()`.

`SettlementCommand` is the resolved, internal form produced by validation and
executed by `World::execute_settlement_command`. It is also used directly on a
`World` for scenario setup (for example `SetBuildingActive` or an explicit load),
but `Simulation` accepts only decisions. Productivity configuration, initial
inventory seeding, direct building insertion and the building catalog are
scenario setup operations, not actions.

## Execution, errors and atomicity

Unknown settlement IDs are rejected when queueing. Before consuming any queued
decisions, `step` checks counter overflow and all existing project memberships.
A preflight error preserves world state, pending decisions and the counter.

In the `Decisions` phase, settlements run in ascending ID order and each
settlement's decisions in submission order. `World::apply_decision` validates a
decision against the current state, then executes its accepted actions in order.
A later decision of the same settlement therefore sees the effects of the earlier
one (for example, it can cancel a project the earlier one created).

Each decision produces a `DecisionOutcome { settlement_id, results }` with one
`ActionResult { index, kind, outcome }` per action in index order. The outcome is
`Applied { effect }` (`ProjectQueued`, `ProjectCanceled`, `WorkersSet`,
`StorageOrderSet`, `MarketOffersPublished`, `MarketOffersCleared`,
`CaravanOrdered`) or `Rejected { error }` with an `ActionError`. Rejected actions
change nothing and do not stop later actions, decisions or phases. Decisions are
consumed once. `ExecutionFailed` would mean that the engine refused a validated
action; it is a debug assertion and not expected in practice. All result types
serialize to JSON. Checked-arithmetic panics are not recoverable turn errors.

## Reports and transient values

`SimulationTurnReport` contains:

- `turn: u64`: just-completed turn, starting at 1.
- `decisions: Vec<DecisionOutcome>`: per-action results, in settlement-ID/submission order.
- `settlements: BTreeMap<SettlementId, TurnReport>`: local accounting and project outcomes.
- `execution: Vec<PhaseExecution>`: actual phase/settlement invocation order.

Local reports contain population consumption, project outcomes, material accounting
and `TradeTurnReport`. Material balance includes trade reservations/receipts; gold
and cargo in transit are outside its scope. Reports do not retain completed-project
history or world knowledge. Transport runs around the seven traced settlement
phases; see [trading](../systems/trading.md).

`PreparedTurn`, `AllocatedTurn`, `ConsumedTurn` and `ProducedTurn` are private
transient phase values moved between calls. They carry allocations and reports,
not another persistent copy of settlement state. A local `Settlement::process_turn`
uses these same methods with an environment and visitor slice, without incrementing
the global counter or moving/dispatching global caravans.

## Headless execution

```sh
cargo run -p simulation_core --example headless -- 100
```

The example creates a small farm scenario directly in Rust, executes the requested
turn count and drops each report after use. The optional count defaults to 10.
It initializes neither Macroquad nor a graphical window.

Sources: [coordinator](../../../core/src/simulation/mod.rs),
[decision dispatch](../../../core/src/simulation/decisions.rs),
[step](../../../core/src/simulation/turn.rs),
[report types](../../../core/src/simulation/turn_report.rs),
[example](../../../core/examples/headless.rs).
