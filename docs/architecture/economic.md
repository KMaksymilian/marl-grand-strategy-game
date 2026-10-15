# Economy: Resources, Allocation and Production

This document describes the implementation including the first economy fixes in `fix/economy` (`9d24d16`) and consistent population demand evaluation. Its scope is the local economy of one settlement. Trading, currency, transport and AI models remain in the [roadmap](economy_roadmap.md). The settlement structure and turn lifecycle are described in [settlements.md](settlements.md); the rationale is recorded in [decisions](../decisions/0001-economy.md).

## 1. Resources and Units

| Type     | Role                                                                       | Uses warehouse capacity |
| -------- | -------------------------------------------------------------------------- | ----------------------- |
| `Wood`   | Recipe input, project cost or consumption good, depending on configuration | Yes                     |
| `Labour` | Available workers for the turn                                             | No, in `total_amount()` |

Material roles come from recipe definitions, population needs and project costs

Inventories, grants, recipe inputs/outputs, material costs and per-capita needs all use `u32` in the same whole resource units. A value of `1` means one unit everywhere. There is no fixed-point scale or conversion between these components. Target efficiency and satisfaction remain dimensionless `f32` coefficients, not resource amounts. Allocation uses `u128` for intermediate sums and products. Production combines repeated inputs using `u64` and computes its scale as a fraction using `u128`.

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

Returns are admitted before new products. Each return, buffer insertion and buffer-to-warehouse transfer processes materials by the settlement's ordered material preference list. The default order comes from `ResourceType::MATERIALS`: `Wood`, `Stone`, `Grain`. Stored stock is never evicted to make room for an incoming batch. An accepted addition and its rejection always sum to the requested amount. Buffer and warehouse rejection are reported separately, so already discarded buffer products are not counted again during transfer.

### Settlement Storage Priorities

Each settlement owns an independent `StorageOrder`: a persistent list from first admitted to last. There are no numerical weights or fixed-length array signatures.

```rust
settlement.set_storage_order(vec![ResourceType::Grain, ResourceType::Wood, ResourceType::Stone]).unwrap();
assert_eq!(settlement.storage_order()[0], ResourceType::Grain);
```

`set_storage_order(materials: impl AsRef<[ResourceType]>)` accepts vectors, slices and arrays. Every material from `ResourceType::MATERIALS` must occur exactly once. Duplicate materials, missing materials and `Labour` return `StorageOrderError` without changing the previous list. The catalogue is centralized; adding a material requires extending it, without changing this API's signature. The list is not consumed as a FIFO queue.

Warehouse, production buffer and returned batches use the same order. Changing it affects future admissions without evicting existing stock or changing demand allocation. Returns still precede new products as a turn phase rule. Single-resource deposits have no batch to reorder. The implementation is in [storage_order.rs](../../core/src/domain/economy/storage_order.rs).

The inventory manager reports `InventoryLoss { resource, amount, stage }` for `Returns`, `ProductionBuffer` and `Warehouse`. It accepts aggregate production/returns in `u64`, admitting only what the `u32` inventory can store. Building-manager aggregate totals also use `u64`; no resource unit conversion is introduced. Materials retain their existing whole-unit scale.

Products are visible in the warehouse after the turn ends, but do not participate in that turn's earlier allocation. They can supply consumption, projects or other recipes in the next turn. Products from an earlier recipe also cannot feed a later recipe in the same building during the same turn.

`refresh_labor_capacity(population)` directly sets `Labour` to the population count. Labour is renewed every turn and generic additions can also succeed when the material warehouse is full. Its counter is capped at `u32::MAX`, with any excess reported as rejected by `add`.

Capacity reduction does not destroy existing goods: a reduction below stock returns `CapacityError { requested, stored }`. Resource maps are private. `resources()` returns an immutable view and `amount(resource)` returns zero for missing entries. External callers cannot insert stock directly to bypass capacity checks.

### Material Balance in the Turn Report

`TurnReport.inventory` contains opening/closing material stock, production, internal returns, consumption by population/projects/buildings and stage-specific losses. Opening and closing snapshots include both `main` and the pending buffer, excluding `Labour`. Aggregate report quantities use `u64` in the same resource units; the balance comparison uses `u128`.

For every material, `report.inventory.is_balanced()` checks:

```text
opening stock + produced
    = closing stock + population consumed + project consumed
      + production inputs consumed + losses
```

Unused building grants are subtracted from production input consumption. Returns are an internal transfer, not new production, and must not be added to the left side. Labour is a renewed capacity and is excluded from this material conservation equation. The report describes operations within `process_turn`; additions made outside the turn must be tracked using their own `InventoryAddResult`.

Tests cover all six insertion orders of three materials, configured material orders and invalid lists, independent settlement policies, priority changes without eviction, losses at both storage stages, complete report equality under reversed building order, combined production above `u32`, returned materials without workers and preexisting buffer stock.

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

The settlement gathers requests from population, projects and buildings. The allocator combines all requests from the same recipient for the same resource before computing shares, including multiple active population need tiers.

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

Remaining units go one at a time to recipients with the largest remainders. Equal remainders are resolved using `AllocationRecipient` order: population, buildings by ascending ID, then projects by ascending ID. There are no separate survival, industry or construction priorities. Input request order does not change the result.

| Available resource | Demands                       | Grants                     |
| ------------------ | ----------------------------- | -------------------------- |
| 100 wood           | Buildings 1 and 2: 150 and 50 | 75 and 25                  |
| 1 wood             | Projects 1 and 2: 1 each      | Project 1: 1; project 2: 0 |
| 100 workers        | Buildings 1 and 2: 40 and 80  | 33 and 67                  |


Materials are withdrawn from the warehouse in quantity `B` before grants are published. If withdrawal fails, the allocator publishes no grants for that resource. `Labour` is not physically withdrawn, but total allocated labour does not exceed the available worker count.

Materials are not allocated as complete recipe bundles. A building may receive wood but no labour. Unused material returns at the end of execution; it is not redistributed to other recipients during that turn.

## 5. Building Production

### Demand

A new `Building` has `is_active = true`, `current_workers = 0` and `target_efficiency = 0.0`. Target efficiency must be set before production starts. In `calculate_demand()`, finite efficiency is clamped to 0–1. Calculations use `f64` after converting the stored `f32`.

```text
labour     = floor(max_workers * target_efficiency)
each input = floor(base_input * target_efficiency)
```

Inputs from all recipes are added to the building request. An inactive building produces an empty demand.

### Execution

`resolve_allocation` separates `Labour` from material grants. The worker count is the minimum of granted labour and `max_workers`. An inactive building or one without workers produces nothing and returns its materials.

`BuildingBehavior::Production` recipes run in definition order. Repeated inputs of the same resource are combined within each recipe first. Its scale is:

```text
s = min(current_workers / max_workers,
        remaining_input_1 / base_input_1,
        remaining_input_2 / base_input_2, ...)
input consumption = ceil(base_input * s)
output quantity   = floor(base_output * s)
```


Example: a building has 10 of 10 workers. A recipe requires 10 wood and 5 grain, but grants contain 10 wood and 2 grain. Its scale is 2/5. It consumes 4 wood and 2 grain, produces 8 stone from a base output of 20 and returns 6 wood.


## 6. Population Needs

`NeedTier` stores `min_population` and `needs_per_capita: HashMap<ResourceType, u32>`. `NeedRegistry` selects all tiers whose thresholds are reached. Needs accumulate; a higher tier does not replace a lower one. Thresholds and quantities are supplied through configuration, without a built-in game catalog.

Population uses one shared integer demand calculation for both requests and evaluation. Per-capita amounts for the same resource are summed across all active tiers and multiplied by population, using whole units without floating-point conversion or rounding:

```text
required_resource = population * sum(active per-capita needs for this resource)
```

`generate_requests()` produces one combined `Population` request. Zero quantities are omitted; zero population or no positive demand produces no requests. Checked integer addition and multiplication reject needs exceeding `u32` with an explicit panic instead of wrapping or silently truncating. A richer configuration error API remains future work.

`process_allocation()` evaluates grants against that same integer demand and returns `ConsumptionResult`. The allocator has already withdrawn material grants. Evaluation performs no second withdrawal and caps each resource's contribution at its own demand:

```text
satisfaction_percent = sum(min(granted_resource, required_resource))
                       / sum(required_resource)
```

Zero total demand yields 1.0. Resources granted less than their integer demand enter `missing_resources`, sorted by `ResourceType` order. There is no floating-point tolerance for shortages. Satisfaction totals use `u64`, and the final ratio is returned as `f32` in the range 0–1. The result remains weighted by material quantities, rather than equally by need.

Examples: three residents requiring 2 grain each request 6 grain, and receiving it yields 1.0. Two active tiers requiring 1 and 2 grain per resident yield 9 grain for three residents. If demand is 1 grain and 1 wood, receiving 2 grain but no wood yields 0.5: excess of one good cannot compensate for another missing good. Unrequested resources do not contribute to satisfaction.

Per-capita needs must currently be whole numbers; fractional rates such as 0.5 are not supported. Any future finer unit scale must apply consistently to all quantities. Population remains unchanged by evaluation. This change does not introduce growth, survival priorities or a new need-weighting policy.

## 7. Sources and Verification

| Area | Code |
| --- | --- |
| Resources and capacity | [resource.rs](../../core/src/domain/economy/resource.rs) |
| Recipients and remainders | [resource_allocation.rs](../../core/src/domain/economy/resource_allocation.rs) |
| Production | [buildings/core.rs](../../core/src/domain/buildings/core.rs) |
| Definitions and factory | [factory.rs](../../core/src/domain/buildings/factory.rs) |
| Population needs | [demographics.rs](../../core/src/domain/settlement/demographics.rs) |
| Warehouse and buffer | [settlement_inventory_manager.rs](../../core/src/domain/settlement/settlement_inventory_manager.rs) |


Run verification with `cargo test --workspace` and `cargo clippy --workspace --all-targets --all-features -- -D warnings`.

### Definition Validation

`BuildingDefinition::validate()` checks per-resource input and output totals across all recipes, including duplicate entries, using checked `u32` arithmetic. Recipe entries cannot contain `Labour`, which is supplied through `max_workers`. Each stat modifier must fit `u32` at maximum staffing. Invalid definitions return `DefinitionError` before registration, construction, upgrade or project queueing. Registration and upgrade preserve existing state on failure. `Building::new()` and `BuildingFactory::register()` now return `Result`; callers must handle invalid configuration. Upgrades clamp current workers to the new staffing limit.

Definitions and project costs remain editable configuration drafts. Live building state is private, and its shared definition is exposed only as `&BuildingDefinition`. Turn reports remain detached, publicly readable data.
