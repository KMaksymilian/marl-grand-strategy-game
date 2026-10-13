# 0001 — Local Economy and Turn Accounting

Date: October 13, 2026. Status: record of implemented solutions, including `fix/economy` (`9d24d16`) and the subsequent population needs repair. Open questions are listed separately and are not presented as approved game rules.

Related documents: [economy](../architecture/economic.md), [settlements](../architecture/settlements.md), [roadmap](../architecture/economy_roadmap.md).

## Context

A settlement must distribute limited materials among population, buildings and projects. Warehouse quantities are integers. Each phase requires an unambiguous grant backed by warehouse stock. Production and construction can change the state only after requests and their allocation have been established.

The first implementation used a shared `u32` recipient ID and `f32` ratios. This could merge project and building grants, allocate large quantities without successful withdrawal and leave available materials unused after rounding. Recipes consulted the original grant even when earlier recipes had consumed it.

## D1. Typed IDs and Separate Recipient Namespaces

**Implemented:** `BuildingId` and `ProjectId` wrap `u32`. `AllocationRecipient` distinguishes `Population`, `Building(BuildingId)` and `Project(ProjectId)`. IDs are local to a settlement.

**Rationale:** the allocation key identifies the recipient kind. A project and building can share a number without sharing a grant. Argument types detect confusion between a project and an upgrade target at compile time.

**Consequences:** managers reject active duplicates within their own kind. Completed construction receives the first free building ID instead of inheriting the project number. Command callers handle `Result`. Fields remain public, so direct mutation can bypass validation.

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

## Open Questions

| Topic | Current behavior | Decision needed |
| --- | --- | --- |
| Need weights | Evaluation is weighted by material quantities | Whether to add weights per need or survival priority |
| Ties across turns | Fixed recipient order | Whether to rotate precedence |
| Warehouse overflow | Excess is discarded; `HashMap` order affects selection | Loss policy and reporting of discarded quantities |
| Project duration | Decreases without materials | Elapsed time since ordering or work time after funding |
| Zero costs | A zero-valued entry can block completion | Cost normalization |
| Invalid upgrades | Projects disappear even without a target | Command validation and refund rules |
| Quantity limits | State and many aggregate quantities use `u32` | Definition validation and boundary behavior |
| Public state | Manager validation can be bypassed | Controlled operations and population API exports |

## Evidence in Code

- [ids.rs](../../core/src/domain/ids.rs) and [resource_allocation.rs](../../core/src/domain/economy/resource_allocation.rs): D1–D3.
- [buildings/core.rs](../../core/src/domain/buildings/core.rs): D4.
- [settlement/core.rs](../../core/src/domain/settlement/core.rs) and [settlement_inventory_manager.rs](../../core/src/domain/settlement/settlement_inventory_manager.rs): D5.
- [demographics.rs](../../core/src/domain/settlement/demographics.rs): D6, including integer needs, tier aggregation, large quantities and excess-grant tests.
- [turn_tests.rs](../../core/src/domain/settlement/turn_tests.rs): ID collisions, single-material allocation, large costs, shared inputs and products becoming usable in the next turn.

Update this record and the architecture documents when the code changes, especially tie rules, recipe order or project duration semantics.
