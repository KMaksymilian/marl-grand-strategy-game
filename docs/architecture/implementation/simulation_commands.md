# Simulation Commands and Reports

The [global lifecycle](../overview/turn_lifecycle.md) defines phase order and the
[registry system](../systems/settlement_registry.md) defines identity/registration.
This reference describes the implemented coordinator API.

## Agent API

The agent reads a snapshot, submits a batch of actions and receives the outcomes
when the caller advances the global turn.

| Step | Method | Return value |
| --- | --- | --- |
| Read | `observe(settlement_id)` | `Result<SettlementObservation, WorldError>` with the settlement's economy, territory and latest local report. The snapshot supports `to_json()`. |
| Submit | `queue_decision(decision)` | `Result<(), WorldError>`. Success means the decision is queued. An unknown settlement is rejected immediately. |
| Advance | `step()` | `Result<SimulationTurnReport, SimulationError>`. The report contains each action's effect or rejection reason. |

For example, with a configured simulation and a registered settlement:

```rust
use simulation_core::ids::BuildingTypeId;
use simulation_core::simulation::{SettlementAction, SettlementDecision};
use simulation_core::spatial::Position;

let observation = simulation.observe(settlement_id)?;
let decision = SettlementDecision::new(
    observation.settlement_id,
    vec![SettlementAction::Construct {
        building_type: BuildingTypeId::new("farm"),
        position: Position { x: 3, y: 4 },
    }],
);
simulation.queue_decision(decision)?;
let report = simulation.step()?;
```

An applied `Construct` returns `ProjectQueued` with the new project ID. An
occupied tile yields `Rejected` with `TileOccupied`. The caller can submit
decisions for several settlements before calling `step()` once for all of them.
`SettlementDecision` and `DecisionOutcome` support JSON. The complete
`SimulationTurnReport` is currently a Rust type without JSON serialization.

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
| `step()` | Delegate to the turn coordinator, store the new turn and reports, and return `SimulationTurnReport`. |
| `observe(id)`, `observe_all()` | Build agent [observations](settlement_observations.md) from the current state and the latest turn reports. |
| `into_world()` | Consume the coordinator and recover its world; pending decisions are discarded. |

The registry retains its ID-allocation cursor in the recovered world. Creating a
new coordinator resets global time to zero; this is not a resume/checkpoint API.
No public mutable world getter allows callers to bypass coordinated mutation.

## Decision contracts

Agents submit a serializable [`SettlementDecision`](settlement_decisions.md):
a settlement ID and a list of `SettlementAction`s (construct, upgrade, cancel,
set workers, storage order, publish/clear market offers, order caravan). Agents
refer to catalog types by ID. The simulation resolves definitions and costs
from `World::building_catalog()` and assigns IDs to new projects through
`Settlement::next_project_id()`. Cancellation refers to an existing project ID.

`SettlementCommand` is the resolved form produced during preparation and
executed by the internal command dispatcher after validation. Scenario setup
uses explicit domain operations on `World`, `Settlement` and `Building`. Productivity configuration, initial
inventory seeding, direct building insertion and the building catalog are
scenario setup operations, not actions.

## Execution, errors and atomicity

Unknown settlement IDs are rejected when queueing. Before consuming any queued
decisions, `step` checks counter overflow and all existing project memberships.
A preflight error preserves world state, pending decisions and the counter.

In the `Decisions` phase, settlements run in ascending ID order and each
settlement's decisions in submission order. The handler checks the input fields,
prepares the decision against the current state and executes accepted actions.
A later decision of the same settlement therefore sees the effects of the earlier
one (for example, it can cancel a project the earlier one created).

Each decision produces a `DecisionOutcome { settlement_id, results }` with one
`ActionResult { index, kind, outcome }` per action in index order. The outcome is
`Applied { effect }` (`ProjectQueued`, `ProjectCanceled`, `WorkersSet`,
`StorageOrderSet`, `MarketOffersPublished`, `MarketOffersCleared`,
`CaravanOrdered`) or `Rejected { error }` with an `ActionError`. Rejected actions
change nothing and do not stop later actions, decisions or phases. Decisions are
consumed once. `ExecutionFailed` reports an operation that the engine refused
after preparation. Per-action results serialize to JSON.

## Reports and transient values

`SimulationTurnReport` contains:

- `turn: u64`: just-completed turn, starting at 1.
- `decisions: Vec<DecisionOutcome>`: per-action results, in settlement-ID/submission order.
- `settlements: BTreeMap<SettlementId, TurnReport>`: local accounting and project outcomes.
- `execution: Vec<PhaseExecution>`: actual phase/settlement invocation order.

The full report implements `serde::Serialize`. A caller can export it with
`serde_json::to_writer(writer, &report)`. JSON settlement keys are string forms
of the numeric settlement IDs, for example `"1"`. Phase names use snake case.
The application layer returns the report. The caller chooses the output
destination and when to write it.

Local reports contain population consumption, project outcomes, material accounting
and `TradeTurnReport`. Material balance includes trade reservations/receipts; gold
and cargo in transit are outside its scope. Reports do not retain completed-project
history or world knowledge. Transport runs around the seven traced settlement
phases; see [trading](../systems/trading.md).

`PreparedTurn`, `AllocatedTurn`, `ConsumedTurn` and `ProducedTurn` are private
transient phase values moved between calls. They carry allocations and reports,
not another persistent copy of settlement state. Tests under `core/src/test_support`
exercise these phases in isolation. Their local turn and decision helpers are
compiled only for tests.

## Headless execution

```sh
cargo run -p simulation_core --example headless -- 100
```

The example creates a small farm scenario directly in Rust, executes the requested
turn count and prints a summary. The optional count defaults to 10. To stream
one full JSON report per turn to standard output, use:

```sh
cargo run -p simulation_core --example headless -- 100 --jsonl > turns.jsonl
```

Both modes run without Macroquad or a graphical window.

Sources: [coordinator](../../../core/src/application/simulation/mod.rs),
[command handler](../../../core/src/application/commands/settlement/handler.rs),
[step](../../../core/src/application/simulation/turn_coordinator.rs),
[report types](../../../core/src/application/simulation/turn_report.rs),
[example](../../../core/examples/headless.rs).
