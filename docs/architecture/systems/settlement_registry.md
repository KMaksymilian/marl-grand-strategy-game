# Settlement Registry

`World` owns one `SettlementRegistry`; `Simulation` owns it through that world.
It stores actual settlement instances in `BTreeMap<SettlementId, Settlement>`.




## Class view

```mermaid
classDiagram
    class SettlementId {
        +u32 value
    }
    class SettlementRegistry {
        -BTreeMap settlements
        -Option~SettlementId~ next_id
        +get(id)
        +iter()
        +len()
        +is_empty()
    }
    Simulation *-- World
    World *-- SettlementRegistry
    World ..> SettlementId : province index
    SettlementRegistry *-- Settlement
    Settlement --> SettlementId : immutable identity
```


## Initialization sequence

```mermaid
sequenceDiagram
    actor Caller
    participant Sim as Simulation
    participant W as World
    participant R as SettlementRegistry
    Caller->>Sim: initialize_settlement(province, capacity, population, needs)
    Sim->>W: initialize_settlement(...)
    W->>R: next_id()
    R-->>W: unused candidate or exhaustion
    alt Exhausted IDs
        W-->>Sim: SettlementIdExhausted
    else Candidate available
        W->>W: construct empty Settlement and validate registration
        alt Invalid or occupied province
            W-->>Sim: error, registry and province index unchanged
        else Valid registration
            W->>R: register(settlement)
            R->>R: store instance, skip occupied ID candidates
            W->>W: record province-to-settlement link
            W-->>Sim: SettlementId
        end
    end
    Sim-->>Caller: result
```

`add_settlement(province_id, settlement)` is the alternative for explicitly chosen
IDs and pre-seeded inventories. It rejects duplicate IDs, unknown/occupied provinces
and settlements already containing buildings/projects.

## Stable access

- Registry, world and simulation iteration follow ascending `SettlementId`.
- Global phases and report keys use that same order.
- `Simulation::settlement(id)` returns `MissingSettlement` for an unknown ID.
- Registry/world lookups return an optional shared reference.
- Public registry access cannot mutate IDs, manager collections or province links.

State persists in memory across turns; no disk persistence or serialization is
implemented. For implementation methods see the
[source map](../implementation/code_map.md) and
[registry code](../../../core/src/domain/world/settlement_registry.rs).
