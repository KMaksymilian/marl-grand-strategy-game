# 0001 — Local Economy and Turn Accounting

Date: October 13, 2026. Status: record of implemented solutions, including `fix/economy` (`9d24d16`), integer population needs and validated project reporting. Open questions are listed separately and are not presented as approved game rules.

Related documents: [economy](../architecture/economic.md), [settlements](../architecture/settlements.md), [roadmap](../architecture/economy_roadmap.md).

## Context

A settlement must distribute limited materials among population, buildings and projects. Warehouse quantities are integers. Each phase requires an unambiguous grant backed by warehouse stock. Production and construction can change the state only after requests and their allocation have been established.

The first implementation used a shared `u32` recipient ID and `f32` ratios. This could merge project and building grants, allocate large quantities without successful withdrawal and leave available materials unused after rounding. Recipes consulted the original grant even when earlier recipes had consumed it.

## D1. Typed IDs and Separate Recipient Namespaces

**Implemented:** `BuildingId` and `ProjectId` wrap `u32`. `AllocationRecipient` distinguishes `Population`, `Building(BuildingId)` and `Project(ProjectId)`. IDs are local to a settlement.

**Rationale:** the allocation key identifies the recipient kind. A project and building can share a number without sharing a grant. Argument types detect confusion between a project and an upgrade target at compile time.

**Consequences:** managers reject active duplicates within their own kind. Completed construction receives the first free building ID instead of inheriting the project number. Command callers handle `Result`. Runtime fields are now protected by D10.

**Alternatives:** a shared `EntityId` would be valid with a common uniqueness generator and entity registry. No such registry exists currently. Reserved numeric ranges provide no type protection and require additional agreements about range boundaries.

## D2. Proportional Allocation with Largest Remainders

**Implemented:** demands for the same resource and recipient are combined before allocation. The budget is the minimum of stock and total demand. Base shares use integer division; remaining units go to the largest remainders. Intermediate sums and products use `u128`.

**Rationale:** grants use the available budget exactly without converting quantities to `f32`. Combining requests prevents artificial splitting of a need from affecting allocation.

**Consequences:** one material unit and two demands of one allow one recipient to advance. Ties are resolved in population, building-ID and project-ID order. Results do not depend on input request order. There is no rotating precedence between turns or explicit economic priority system. Allocation does not optimize complete material bundles for recipes.

**Alternatives:** flooring every proportional share can leave unused units and stall progress. Fractional quantities would require changing inventory and cost representations. Random tie resolution requires explicit seed control and a different repeatability contract.

## D3. Material Grants Require Successful Withdrawal

**Implemented:** the allocator withdraws the material budget before publishing grants and checks `consume`. `Labour` is an exception: its stock represents renewable work capacity rather than a physical material.

**Rationale:** recipients spend materials already assigned to them instead of withdrawing again from the shared warehouse. This reduces the risk of double charging or grants without stock coverage.

**Consequences:** population evaluates its grant, projects reduce costs, and buildings return unused materials. Total labour grants are limited by population even though the `Labour` warehouse value does not decrease. Returns are processed at the end of execution without reallocating them during the same turn.

## D4. Recipes Use the Remaining Balance

**Implemented:** definition `behaviors` order determines recipe order. Repeated inputs of one resource are aggregated within a recipe. The scale is an exact fraction constrained by workers and remaining inputs. Input consumption rounds up; products round down.

**Rationale:** materials consumed by an earlier recipe are unavailable to later recipes. Exact withdrawal calculations eliminate floating-point rounding that could exceed the balance.

**Consequences:** the first recipe takes precedence when shared materials are scarce. Each recipe uses the same building workforce; labour is not divided between recipes. Definition order affects results and should be configured deliberately. There is no proportional material sharing between recipes or optimization of recipe combinations.

## D5. Turn Phases and Buffered Production

**Implemented:** all requests are gathered before economic execution. Products enter the buffer and reach `main` only after grants have been processed. Project finalization follows production.

**Rationale:** current production cannot directly feed production chains or needs during the same turn. A new building cannot participate in requests gathered before its creation.

**Consequences:** products and new buildings affect future allocation. The buffer and warehouse have limits; phase separation does not determine a fair or repeatable choice of materials under overflow. The buffer is drained at turn end rather than retained until the next turn begins.

## D6. Shared Integer Resource Units and Population Demand

**Implemented:** per-capita needs use `u32`, matching inventory quantities, recipe inputs/outputs and project costs. One represents one whole resource unit throughout the economy. Combine needs across active tiers and multiply by population with checked integer arithmetic. Request generation and evaluation use the same calculation. Population generates one combined request, omitting zero demands. Each resource contributes at most its required quantity to satisfaction; shortages are detected by integer comparison.

**Rationale:** population needs previously used `f32` while buildings and projects used integers. This required conversions and could produce satisfaction above 100%. Using the existing resource unit throughout removes these conversions at minimal implementation cost. Uncapped grants could also conceal a shortage of another good.

**Consequences:** full grants yield 1.0; excess of one material cannot replace another. Zero demand yields 1.0 without requests. The score retains quantity weighting, and its aggregate totals use `u64`. Missing resources have a stable order. Fractional per-capita rates are not supported. No fixed-point scale, display conversion or quantity wrapper is introduced. Needs above the `u32` range are rejected by checked arithmetic with an explicit panic. Efficiency and satisfaction remain dimensionless coefficients; no growth, demographic penalties or alternative need weights are introduced.

**Example:** three residents with grain needs of 1 and 2 across active tiers request 9 grain. Warehouse stock, project costs and recipe inputs use the same unit. Demand of 1 grain and 1 wood with a grant of 2 grain and no wood yields 0.5.

## D7. Normalize Costs, Validate Effects and Report Project Outcomes

**Implemented:** remove zero material costs at project creation, registration and allocation. Zero entries are excluded from requests and do not prevent a fully paid cost. `queue_upgrade` validates its target before registering a command. Active projects are validated again before allocation; unsupported recruitment or a missing target is rejected without further grants. Completed project effects are also checked and their results handled explicitly.

**Rationale:** a zero-valued cost entry could leave a project stuck, while invalid upgrades could consume materials and disappear without an effect. Validation must happen before resource allocation rather than only after payment.

**Consequences:** `process_turn()` now returns a public `TurnReport` with `consumption` and `projects`. Each removed project produces one `ProjectOutcome` containing its ID and either a completion with a building ID or a typed error. Pending projects produce no event. Queue errors are returned immediately as `ProjectError`, and duplicate registration remains guarded. Unsupported recruitment does not charge materials. Active effects are checked every turn; D10 additionally protects collections from external mutation.

Previously paid material costs are not refunded if a target later disappears. Project time still decreases without material grants. Refunds, cancellation and timing policy remain separate decisions. D8 extends the initial report with production, returns and warehouse losses.

## D8. Deterministic Storage, Explicit Losses and Material Balance

**Implemented:** `add()` returns accepted and rejected quantities. Returns are processed before products; the initial storage policy used ascending resource order, subsequently replaced by D9's configurable priorities. Excess is discarded with a stage-specific loss record rather than silently. Buffer losses and warehouse losses are separate. `Labour` does not use material capacity. Capacity is private, and `set_capacity()` rejects shrinking below stock without deleting goods. Free-space subtraction is saturating to handle internally inconsistent resource maps safely.

**Rationale:** `HashMap` iteration previously selected which goods survived overflow, making repeated simulations diverge. Silent rejection prevented full accounting, and a capacity below public stock could underflow during an addition.

**Consequences:** the default material order is `Wood`, `Stone`, `Grain`; D9 retains it for equal-priority ties. Physical inventories, costs and requests retain `u32` whole-unit amounts. Multi-building aggregate production/returns and report totals use `u64` to account for excess without wrapping. `TurnReport.inventory` records stock, produced goods, returned grants, consumption categories and losses. Its balance check excludes labour and does not count internal returns as production. Opening/closing stock includes the pending buffer.

Existing stock is retained. No eviction or automatic shrinking loss is introduced. Warehouse and buffer capacities can be changed independently through their setters. Public resource maps still permit invalid external state, so full state encapsulation remains future work. Per-building definition limits remain outside this stage.

## D9. Storage Priorities Owned by Each Settlement

**Superseded:** numerical storage weights have been replaced by an ordered material list (D10 below).

**Rationale:** fixed resource order systematically preferred wood during overflow regardless of settlement needs. Explicit settlement priorities allow the caller or a future agent to choose which incoming materials to retain without adding probabilistic behavior or a new quantity scale.

**Consequences:** the same policy applies to returns, the production buffer and warehouse transfer. Phase precedence remains returns before new products; existing stock is not evicted. Priorities affect storage admissions, not demand allocation or warehouse size. Changes apply to later batches. Equal priorities retain deterministic tie handling and the all-zero default preserves previous behavior. No automatic policy, per-resource capacity limits or stock replacement is introduced.

## Open Questions

| Topic | Current behavior | Decision needed |
| --- | --- | --- |
| Need weights | Evaluation is weighted by material quantities | Whether to add weights per need or survival priority |
| Ties across turns | Fixed recipient order | Whether to rotate precedence |
| Storage policy selection | Each settlement explicitly sets priorities; no eviction | Whether to add automatic policies or replacement of existing stock |
| Project duration | Decreases without materials | Elapsed time since ordering or work time after funding |
| Failed projects and refunds | Invalid effects are reported before further spending; earlier costs remain spent | Refund and cancellation rules |
| Quantity limits | State and many aggregate quantities use `u32` | Definition validation and boundary behavior |
| Public state | Manager validation can be bypassed | Controlled operations and population API exports |

## Evidence in Code

- [ids.rs](../../core/src/domain/ids.rs) and [resource_allocation.rs](../../core/src/domain/economy/resource_allocation.rs): D1–D3.
- [buildings/core.rs](../../core/src/domain/buildings/core.rs): D4.
- [settlement/core.rs](../../core/src/domain/settlement/core.rs) and [settlement_inventory_manager.rs](../../core/src/domain/settlement/settlement_inventory_manager.rs): D5.
- [demographics.rs](../../core/src/domain/settlement/demographics.rs): D6, including integer needs, tier aggregation, large quantities and excess-grant tests.
- [projects.rs](../../core/src/domain/settlement/projects.rs), [turn_report.rs](../../core/src/domain/settlement/turn_report.rs) and [settlement/core.rs](../../core/src/domain/settlement/core.rs): D7, normalized costs, command validation and project outcomes.
- [resource.rs](../../core/src/domain/economy/resource.rs), [settlement_inventory_manager.rs](../../core/src/domain/settlement/settlement_inventory_manager.rs) and [turn_report.rs](../../core/src/domain/settlement/turn_report.rs): D8, capacity validation, loss reporting and material balance.
- [storage_order.rs](../../core/src/domain/economy/storage_order.rs) and [settlement/core.rs](../../core/src/domain/settlement/core.rs): D9/D10, explicit settlement storage order.
- [turn_tests.rs](../../core/src/domain/settlement/turn_tests.rs): ID collisions, single-material allocation, large costs, shared inputs and products becoming usable in the next turn.

Update this record and the architecture documents when the code changes, especially tie rules, recipe order or project duration semantics.

## D10. Ordered Storage Preferences and Protected Runtime State

Storage preferences use a variable-length ordered list, without numerical weights or fixed-size array APIs. It must contain each material in the centralized `ResourceType::MATERIALS` catalogue exactly once. Validation is atomic. This keeps caller and future AI actions expressed as resource orderings, while preserving admission behavior and the returns-first phase rule.

Building definitions are validated at registration and every application boundary. Per-resource recipe totals and maximum-staffed stat modifiers must fit `u32`; labour belongs to staffing, not recipe material entries. Target efficiency must be finite and within 0–1. Invalid updates preserve the previous valid state.

Inventory maps and building fields are private. Settlement owns private managers; their internal collections are restricted to the settlement implementation. Public collection views are immutable. Controlled commands enforce capacity, ID uniqueness and definition validity. Configuration drafts and detached reports remain ordinary public data. No fixed-point conversion, interpreter or new simulation dependency is introduced.

## D11. Keep Commands on Settlement

`Settlement` remains the public local interface and owns private simulation state. Commands and immutable views are grouped in a second `impl Settlement` in the private child module `commands.rs`. This separates files without a forwarding object, copied state or extra lifetime parameters for callers. Unit tests inspecting private state stay inline; public integration tests belong in `core/tests/`. Developed separately on `feature/settlement-api`.
