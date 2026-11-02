# Settlement Economy

The economy distributes limited integer quantities among population, buildings
and projects.

## Class view

```mermaid
classDiagram
    Settlement *-- SettlementInventoryManager
    SettlementInventoryManager *-- ResourceInventory : main
    SettlementInventoryManager *-- ResourceInventory : pending_production
    SettlementInventoryManager *-- StorageOrder
    class ResourceInventory {
        -resources
        -u32 max_capacity
        +add(resource, amount)
        +consume(resource, amount)
        +set_capacity(capacity)
    }
    class StorageOrder {
        -Vec materials
        +set(materials)
        +get()
    }
    AllocationEngine ..> ResourceInventory : withdraw material budget
    AllocationEngine ..> DemandRequest
    DemandRequest --> AllocationRecipient
    InventoryTurnReport ..> ResourceInventory : opening and closing snapshots
```

`Labour` is renewable capacity derived from population, not a stored physical good consuming space.

## Resource activity

```mermaid
flowchart TD
    Stock[Opening warehouse stock] --> Allocate[Allocate material budget once]
    Needs[Combined population, project and building requests] --> Allocate
    Labour[Renewed labour capacity] -.-> Allocate
    Allocate --> People[Population consumption]
    Allocate --> Projects[Project material payment]
    Allocate --> Buildings[Building inputs and staffing]
    Buildings --> Unused[Unused material grants]
    Buildings --> Output[Produced goods]
    Unused --> ReturnAdmission[Ordered return admission to warehouse]
    ReturnAdmission --> ReturnLoss[Report rejected returns]
    ReturnAdmission --> Closing[Closing material stock]
    Output --> Buffer[Ordered production-buffer admission]
    Buffer --> BufferLoss[Report buffer rejection]
    Buffer --> Transfer[Ordered buffer-to-warehouse transfer]
    Transfer --> WarehouseLoss[Report warehouse rejection]
    Transfer --> Closing
    Closing --> Next[Available to the next turn's allocation]
```

Each addition divides its input into accepted and rejected quantities. Rejections are losses.

## Allocation and production rules

Allocation is proportional by demand, with largest remainders distributing whole
units. Equal remainders use fixed recipient order: population, building IDs, then
project IDs.

Buildings run recipes in definition order against remaining material grants.
Inputs round up and outputs round down using exact execution fractions. Unused
grants return at turn end and are not reallocated within that turn.
