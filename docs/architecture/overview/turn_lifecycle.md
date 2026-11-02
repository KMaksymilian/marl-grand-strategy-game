# Global Turn Lifecycle

One successful `Simulation::step()` advances the global counter once. Every phase
runs for all registered settlements in ascending `SettlementId` order before the
next phase begins. The order comes from the registry, not hash-map iteration.

## Phase order

| Phase | Effect |
| --- | --- |
| Decisions | Apply queued commands in submission order within each settlement; report individual rejection. |
| Preparation | Refresh productivity, snapshot opening stock, reject invalid projects, refresh labour, reserve market stock, fund caravan orders and match arrived visitors. |
| Allocation | Gather demands and withdraw material budgets before publishing grants. |
| Consumption | Evaluate population allocations and shortages. |
| Construction | Pay project grants and decrement remaining time once. |
| Production | Execute existing buildings using their allocated inputs and labour. |
| EconomicUpdates | Collect output and trade/internal returns, admit materials, record losses and closing stock, and finalize projects. |

After Decisions, `World::begin_trade_turn` moves existing caravans and queues returns.
After EconomicUpdates, `World::finish_trade_turn` routes funded departures. These
transport boundaries are not separate `TurnPhase` entries. New caravans first
move next turn. See [trading](../systems/trading.md).

## Global sequence



```mermaid
sequenceDiagram
    actor Caller
    participant Sim as Simulation
    participant W as World
    participant S as Settlement
    Caller->>Sim: queue_decision(id, command)
    Sim-->>Caller: queued or MissingSettlement
    Caller->>Sim: step()
    Sim->>Sim: check counter overflow
    loop Every registered SettlementId
        Sim->>W: validate_settlement_turn(id)
    end
    alt Preflight fails
        Sim-->>Caller: SimulationError, world and queue unchanged
    else Preflight succeeds
        Sim->>W: execute queued decisions
        Sim->>W: begin_trade_turn()
        loop Each remaining global phase, in table order
            loop Every SettlementId in ascending order
                Sim->>W: borrow target and temporary environment for phases
                Sim->>S: execute shared phase through capabilities
                S-->>Sim: outcome or transient phase state
            end
        end
        Sim->>W: finish_trade_turn(default locations, finder)
        Sim->>Sim: increment current_turn once
        Sim-->>Caller: SimulationTurnReport
    end
```

Allocation occurs before consumption, construction and production. New products
cannot satisfy this turn's earlier requests. Construction/upgrade finalization
occurs after production, so new definitions affect later turns.

## Execution state view

These are conceptual execution stages of `step`.

```mermaid
stateDiagram-v2
    [*] --> Ready
    Ready --> Preflight: step()
    Preflight --> Ready: error / preserve world, queue and counter
    Preflight --> Decisions: valid
    Decisions --> TransportStart: all commands attempted
    TransportStart --> Preparation: move caravans and queue returns
    Preparation --> Allocation: all settlements prepared
    Allocation --> Consumption: all grants assigned
    Consumption --> Construction: all needs evaluated
    Construction --> Production: all projects progressed
    Production --> EconomicUpdates: all buildings executed
    EconomicUpdates --> Dispatch: all settlements finished
    Dispatch --> Ready: route departures, increment turn and return report
    note right of Decisions
        Invalid commands produce outcomes.
        They do not abort the turn.
    end note
```

## Transient state and reports

The private phase values are moved between calls:

```mermaid
classDiagram
    PreparedTurn ..> AllocatedTurn : allocate_turn
    AllocatedTurn ..> ConsumedTurn : consume_turn
    ConsumedTurn ..> ConsumedTurn : progress_construction borrows grants
    ConsumedTurn ..> ProducedTurn : produce_turn
    ProducedTurn ..> TurnReport : finish_turn
    SimulationTurnReport *-- TurnReport : by SettlementId
    SimulationTurnReport *-- DecisionOutcome
    SimulationTurnReport *-- PhaseExecution
```

`Settlement::process_turn` invokes the same phases for one local turn; `World`
also exposes a local convenience entry point. Calling those directly is not a
global turn and does not advance `Simulation::current_turn`.


API details: [simulation commands](../implementation/simulation_commands.md).
Code: [global pipeline](../../../core/src/simulation/turn.rs),
[shared local phases](../../../core/src/settlement/turn/mod.rs).
