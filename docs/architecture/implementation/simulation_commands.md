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
| `queue_decision(SettlementId, decision)` | Queue one command for a known settlement; no immediate world mutation. |
| `step()` | Execute one global turn and return `SimulationTurnReport`. |
| `into_world()` | Consume the coordinator and recover its world; pending decisions are discarded. |

The registry retains its ID-allocation cursor in the recovered world. Creating a
new coordinator resets global time to zero; this is not a resume/checkpoint API.
No public mutable world getter allows callers to bypass coordinated mutation.

## Decision contracts

`queue_decision` currently takes a `SettlementCommand`. The serializable agent
contract is [`SettlementDecision`](settlement_decisions.md); it is not connected
to the queue yet. `SettlementCommand` supports:

| Variant | Target and effect |
| --- | --- |
| `Construct` | Project ID, position, shared definition and explicit cost; World checks province membership and Settlement reserves the global slot. |
| `Upgrade` | Project ID, local building ID, replacement definition and cost; existing occupancy remains a building. |
| `CancelProject` | Stop an active project and release only its construction reservation; no refund. |
| `SetBuildingLoad` | Validate a finite load in 0..=1 for an existing building. |
| `SetBuildingActive` | Toggle activity for an existing building. |
| `SetStorageOrder` | Validate a complete material ordering without duplicates or labour. |

Costs are caller supplied and are not copied from blueprint `construction_cost`.
Productivity configuration, initial inventory seeding and direct building insertion
are scenario setup operations, not decision variants. There are no AI policy,
trade, transport, population-growth or diplomacy decisions.

## Errors and atomicity

Unknown settlement IDs are rejected when queueing. Before consuming any queued
commands, `step` checks counter overflow and all existing project memberships.
A preflight error preserves world state, pending commands and the counter.

After preflight, each command produces a `DecisionOutcome` containing its typed
settlement ID, original command and `Result<(), DecisionError>`. Failed commands
are consumed once and do not abort later commands or phases. Valid earlier commands
are not rolled back. `DecisionError` wraps world/command errors or invalid storage
order. Checked-arithmetic panics are not recoverable turn errors.

## Reports and transient values

`SimulationTurnReport` contains:

- `turn: u64`: just-completed turn, starting at 1.
- `decisions: Vec<DecisionOutcome>`: outcomes in settlement-ID/submission order.
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
