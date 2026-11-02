# Economy Implementation: Quantities and Algorithms

This document describes the implemented local resource economy of one settlement

## 1. Resources and Units

| Type     | Role                                                                       | Uses warehouse capacity |
| -------- | -------------------------------------------------------------------------- | ----------------------- |
| `Labour` | Available workers for the turn                                             | No, in `total_amount()` |

Material roles come from recipe definitions, population needs and project costs

Inventories, grants, recipe inputs/outputs, material costs and per-capita needs all use `u32` in the same whole resource units. A value of `1` means one unit everywhere. Target load and satisfaction remain dimensionless `f32` coefficients, not resource amounts. Allocation uses `u128` for intermediate sums and products. Production combines repeated inputs using `u64` and computes its scale as a fraction using `u128`.

## 2. Warehouse and Production Buffer

`ResourceInventory` stores a private `HashMap<ResourceType, u32>` and a private shared capacity of type `u32`. Capacity limits the sum of materials, rather than each resource separately. Read it through `capacity()` and change it through `set_capacity()`.

| Method | Behavior |
| --- | --- |
| `new(capacity)` | Creates an empty inventory |
| `total_amount()` | Sums materials in `u64`, excluding `Labour` |
| `capacity()` | Returns the current material limit |
| `set_capacity(capacity)` | Returns `Result<(), CapacityError>`; rejects a limit below stored materials without changing capacity or stock |
| `add(resource, amount)` | Returns `InventoryAddResult { accepted, rejected }`; material additions are limited by free space and `Labour` only by its `u32` counter |
| `consume(resource, amount)` | Withdraws the whole quantity and returns `true`; returns `false` without a partial withdrawal if stock is insufficient |

`SettlementInventoryManager` owns `main` and `pending_production`, initially with the same capacity. Unused material grants return to `main`; products enter the buffer. At the end of turn accounting, `commit_pending_production()` drains the buffer into `main` through `add`.

### Settlement Storage Priorities

Each settlement owns an independent `StorageOrder`: a persistent list from first admitted to last.

`set_storage_order(materials: impl AsRef<[ResourceType]>)` accepts vectors, slices and arrays. Every material from `ResourceType::MATERIALS` must occur exactly once.

Warehouse, production buffer and returned batches use the same order. Changing it affects future admissions without evicting existing stock or changing demand allocation.

Products are visible in the warehouse after the turn ends, but do not participate in that turn's earlier allocation.

`refresh_labor_capacity(population)` directly sets `Labour` to the population count. Labour is renewed every turn.


### Material Balance in the Turn Report

`TurnReport.inventory` contains opening/closing material stock, production, internal returns, consumption by population/projects/buildings and stage-specific losses.
For every material, `report.inventory.is_balanced()` checks:

```text
opening stock + produced
    = closing stock + population consumed + project consumed
      + production inputs consumed + losses
```



## 3. Demand Recipients

```rust
pub struct BuildingId(pub u32);
pub struct ProjectId(pub u32);

pub enum AllocationRecipient {
    Population,
    Building(BuildingId),
    Project(ProjectId),
}

pub struct DemandRequest {
    pub recipient: AllocationRecipient,
    pub demand: HashMap<ResourceType, u32>,
}

pub type AllocationResult =
    HashMap<AllocationRecipient, HashMap<ResourceType, u32>>;
```

IDs are local to a settlement.  A missing recipient or resource in the result means a zero grant.



## 4. Proportional Integer Allocation

`AllocationEngine::execute(&mut inventory, requests)` distributes each resource independently. For one resource, let:

- `d_i` be the combined demand of recipient i;
- `D = sum(d_i)` be total demand;
- `A` be available stock;
- `B = min(A, D)` be the allocation budget.

Zero demands are skipped. A resource is not allocated when `B = 0`. Otherwise:

```text
base share i     = (d_i * B) / D    [integer division]
remainder i      = (d_i * B) % D
remaining budget = B - sum(base shares)
```

Remaining units go one at a time to recipients with the largest remainders. Equal remainders are resolved using `AllocationRecipient` order: population, buildings by ascending ID, then projects by ascending ID.

| Available resource | Demands                       | Grants                     |
| ------------------ | ----------------------------- | -------------------------- |
| 100 wood           | Buildings 1 and 2: 150 and 50 | 75 and 25                  |
| 1 wood             | Projects 1 and 2: 1 each      | Project 1: 1; project 2: 0 |
| 100 workers        | Buildings 1 and 2: 40 and 80  | 33 and 67                  |

## 5. Building Production

### Demand

A new `Building` has `is_active = true`, `current_workers = 0` and `target_load = 0.0`. `set_target_load()` sets the requested operating load and rejects non-finite values or values outside 0–1 without changing state. Load must be set before production starts. Calculations use `f64` after converting the stored `f32`.

```text
labour     = floor(max_workers * target_load)
each input = floor(base_input * target_load)
```

Inputs from all recipes are added to the building request. An inactive building produces an empty demand.

### Execution

`BuildingBehavior::Production` recipes run in definition order. Repeated inputs of the same resource are combined within each recipe first. Its scale is:

```text
s = min(current_workers / max_workers,
        remaining_input_1 / base_input_1,
        remaining_input_2 / base_input_2, ...)
input consumption = ceil(base_input * s)
output quantity   = floor(base_output * s * location_productivity)
```


Example: a building has 10 of 10 workers. A recipe requires 10 wood and 5 grain, but grants contain 10 wood and 2 grain. Its scale is 2/5. It consumes 4 wood and 2 grain, produces 8 stone from a base output of 20 at full location productivity and returns 6 wood. At 50% location productivity it consumes the same materials and produces 4 stone.


## 6. Population Needs

`NeedTier` stores `min_population` and `needs_per_capita: HashMap<ResourceType, u32>`. `NeedRegistry` selects all tiers whose thresholds are reached. Needs accumulate; a higher tier does not replace a lower one.

```text
required_resource = population * sum(active per-capita needs for this resource)
```

`generate_requests()` produces one combined `Population` request.

`process_allocation()` evaluates grants against that same integer demand and returns `ConsumptionResult`.

```text
satisfaction_percent = sum(min(granted_resource, required_resource))
                       / sum(required_resource)
```

## 7. Sources and Verification

| Area | Code |
| --- | --- |
| Resources and capacity | [inventory.rs](../../../core/src/economy/inventory.rs) |
| Recipients and remainders | [allocation.rs](../../../core/src/economy/allocation.rs) |
| Production | [buildings/building.rs](../../../core/src/buildings/building.rs) |
| Definitions and validation | [definition.rs](../../../core/src/buildings/definition.rs) |
| Shared blueprint registry | [catalog.rs](../../../core/src/buildings/catalog.rs) |
| Population needs | [population.rs](../../../core/src/settlement/economy/population.rs) |
| Warehouse and buffer | [inventory_manager.rs](../../../core/src/settlement/economy/inventory.rs) |
