# Source File and Method Map

Implementation reference. Start with the [architecture guide](../README.md) for
system diagrams and boundaries.

This index describes the current Rust workspace. Paths are relative to the
repository root; each file heading links to source. Method names refer to explicit
runtime implementations, including private helpers. Test functions and derived
trait methods are excluded. A `pub` method inside a private manager module is
still an internal implementation detail. See [project structure](project_structure.md)
for module boundaries and import migration, and use `cargo doc --workspace --no-deps`
for full public signatures.

## Entry point and configuration

### [main_app/src/main.rs](../../../main_app/src/main.rs)

`main()` loads `configuration.toml`, logs productivity thresholds, generates the
world, creates the screen and shared pathfinding buffer, and runs the render loop.
It does not execute settlement turns or caravan travel.

### [core/src/configuration.rs](../../../core/src/configuration.rs)

Contains `Configuration`, `EconomyConfiguration`, private TOML DTOs
(`RawConfiguration`, `RawEconomy`, `RawBuildingProductivity`) and `ConfigurationError`.

| Method | Purpose |
| --- | --- |
| `Configuration::load` | Read a file, then parse and validate it. |
| `Configuration::from_toml` | Deserialize strict TOML and validate the travel-cost thresholds. |
| `ConfigurationError::fmt` | Format a readable read, parse or validation error. |
| `ConfigurationError::source` | Expose the underlying I/O or TOML error when present. |

`configuration.toml` supplies full/zero building productivity costs. `Cargo.toml`
at the root declares workspace members; each crate's `Cargo.toml` declares its
package and dependencies. `Cargo.lock` records their resolved versions.

## Simulation coordinator

### [core/src/simulation/mod.rs](../../../core/src/simulation/mod.rs)

Contains `Simulation` and `SimulationError`; `World` remains the only settlement registry.

| Method | Purpose |
| --- | --- |
| `Simulation::new` | Validate map shape, terrain and province IDs; initialize turn zero and search buffers. |
| `current_turn` | Read the number of completed global turns. |
| `world` | Read the owned world without mutable access. |
| `settlements` | Read settlements from the world registry. |
| `settlement_registry` | Read the same ordered persistent registry owned through World. |
| `settlement` | Look up a typed ID or return `MissingSettlement`. |
| `initialize_settlement` | Delegate initialization and unique automatic ID allocation. |
| `add_settlement` | Delegate one-province settlement registration. |
| `queue_decision` | Queue a known settlement's command for the next successful step. |
| `into_world` | Recover the world, discarding pending commands. |
| `SimulationError::fmt` | Format map, counter or preflight failures. |

### [core/src/simulation/decisions.rs](../../../core/src/simulation/decisions.rs)

Re-exports `SettlementCommand` as `SettlementDecision` and world `CommandError` as
`DecisionError`. Defines `DecisionOutcome`; dispatch belongs to `World::execute_settlement_command`.

### [core/src/simulation/turn.rs](../../../core/src/simulation/turn.rs)

`Simulation::step` preflights all settlements, consumes decisions once and executes
each phase globally in ascending settlement ID order before incrementing the counter.

### [core/src/simulation/turn_report.rs](../../../core/src/simulation/turn_report.rs)

Defines `TurnPhase`, `PhaseExecution` and `SimulationTurnReport`; no explicit methods.

## Buildings

### [core/src/buildings/building.rs](../../../core/src/buildings/building.rs)

Runtime `Building`, `LoadError` and
`ProductionResult`. Types are re-exported from `simulation_core::buildings`. This file owns
state, lifecycle, settings and read-only views; recipe processing is a child module.

| Method on `Building` | Purpose |
| --- | --- |
| `new` | Create an active building at zero target load from an already validated blueprint; returns Self. |
| `instance_id`, `position`, `definition` | Read identity, location and the shared blueprint. |
| `target_load`, `location_productivity` | Read desired utilization and the cached location multiplier. |
| `is_active`, `current_workers` | Read activity and staffing from the latest allocation. |
| `set_active` | Enable or disable the building. |
| `set_target_load` | Accept only finite utilization values in `[0, 1]`. |
| `set_location_productivity` | Internally replace the exact cached multiplier. |
| `upgrade` | Replace the validated shared definition while retaining instance state and clamping workers; returns (). |

### [core/src/buildings/building/production.rs](../../../core/src/buildings/building/production.rs)

Additional methods on the same `Building` type, grouped around the production cycle.

| Method on `Building` | Purpose |
| --- | --- |
| `calculate_demand` | Request inputs and labour according to activity and target load. |
| `resolve_allocation` | Set staffing and return ProductionResult with products and unused grants. |
| `execute_production` | Apply worker/material bottlenecks and exact output scaling. |

### [core/src/buildings/definition.rs](../../../core/src/buildings/definition.rs)

Contains `BuildingDefinition`, composed `BuildingBehavior` variants and
`DefinitionError`. `BuildingDefinition::new()` calls private `validate()` to check
recipe sums, forbid labour in material recipes and validate stat arithmetic.
`id`, `name`, `tier`, `max_workers`, `behaviors` and `construction_cost` expose
read-only views; fields are private. Construction returns Result.

### [core/src/buildings/catalog.rs](../../../core/src/buildings/catalog.rs)

`BuildingCatalog` stores blueprints as shared `Arc<BuildingDefinition>` values.
`register()` inserts/replaces an already validated definition by string ID and returns ();
`get_blueprint()` clones its shared reference or returns `None`.

### [core/src/buildings/productivity.rs](../../../core/src/buildings/productivity.rs)

| Type / method | Purpose |
| --- | --- |
| `TravelCostEfficiencyPolicy::new` | Validate a nonempty linear falloff interval. |
| `full_productivity_cost`, `zero_productivity_cost` | Read the configured thresholds. |
| `TravelCostEfficiencyPolicy::productivity` | Convert a route cost or missing route to an exact ratio. |
| `BuildingProductivityCalculator::new` | Bind a capital position to a validated policy. |
| `capital`, `policy` | Read those calculator settings. |
| `travel_cost_to` | Query directed A* cost from capital to a building position. |
| `productivity_at` | Return the multiplier as an inspection-friendly `f64`. |
| `multiplier_at` | Return the exact internal `LocationProductivity`. |
| `LocationProductivity::as_factor` | Convert the exact ratio to `f64`. |
| `LocationProductivity::scale_output` | Combine recipe, work and location ratios, rounding down once. |

`InvalidTravelCosts` reports invalid thresholds. `LocationProductivity::FULL` and
`ZERO` are internal endpoint ratios. See [building productivity](building_productivity.md).

## Settlement


### [core/src/settlement/mod.rs](../../../core/src/settlement/mod.rs)

Owns `Settlement`, `BuildingCommandError`, module visibility and re-exports of
`NeedRegistry` and `NeedTier`. It holds private managers, gold and the trade book.

| Method on `Settlement` | Purpose |
| --- | --- |
| `new` | Create managers, empty inventories/buildings/projects and an empty trade book. |
| `id`, `population` | Read settlement identity and population size. |
| `inventory`, `pending_inventory` | Borrow read-only warehouse and production-buffer views. |
| `buildings`, `active_projects` | Inspect buildings and ongoing projects without mutable collection access. |
| `building_productivity_calculator` | Inspect the optional travel-cost configuration. |
| `storage_order` | Borrow the ordered material storage policy. |


### [core/src/settlement/commands/mod.rs](../../../core/src/settlement/commands/mod.rs)

Declares the three private command groups below; it contains no methods.
Every public command remains a method on the same `Settlement` type.
Private `economy/mod.rs` groups the local managers; `projects/mod.rs` exports
project values and keeps its manager/lifecycle children private. `commands/command.rs`
is exposed through `settlement::command`; `turn/report.rs` through
`settlement::turn_report`. `settlement/stat.rs` is removed; stat targets live in
`buildings::effects`.

### [core/src/settlement/commands/inventory.rs](../../../core/src/settlement/commands/inventory.rs)

Warehouse mutations and storage admission policy.

| Method | Purpose |
| --- | --- |
| `deposit` | Add stock and return accepted/rejected amounts. |
| `withdraw` | Remove the requested amount only if stock suffices. |
| `set_inventory_capacity` | Change capacity, rejecting a limit below stored materials. |
| `set_storage_order` | Validate and replace the complete ordered material list. |

### [core/src/settlement/commands/buildings.rs](../../../core/src/settlement/commands/buildings.rs)

Building registration and operational settings.

| Method on `Settlement` | Purpose |
| --- | --- |
| `add_building` | Validate ID, land and vacant slot, register spatial occupancy and initialize productivity. |
| `set_building_productivity_calculator` | Replace/disable the policy and refresh all building multipliers. |
| `set_building_load` | Find a building and validate its requested utilization. |
| `set_building_active` | Find a building and toggle activity. |

### [core/src/settlement/commands/projects.rs](../../../core/src/settlement/commands/projects.rs)

Requests that create construction/upgrade projects.

| Method on `Settlement` | Purpose |
| --- | --- |
| `queue_construction` | Accept a validated blueprint, check the local project ID, then reserve through ConstructionSpace and register a project. |
| `queue_upgrade` | Check the upgrade target and project ID, then register an upgrade using a validated replacement blueprint. |
| `cancel_project` | Release the exact construction reservation and remove the project; preserve upgrades' buildings and already paid costs. |

### [core/src/settlement/economy/buildings.rs](../../../core/src/settlement/economy/buildings.rs)

Private `BuildingManager` owns instances and the optional productivity calculator.

| Method | Purpose |
| --- | --- |
| `new` | Start with no buildings or productivity policy. |
| `construct_building` | Reject duplicate IDs, calculate productivity and insert a building. |
| `productivity_calculator` | Read the configured calculator. |
| `set_productivity_calculator` | Store the calculator and refresh every building. |
| `refresh_productivity` | Recompute cached multipliers against the current map. |
| `productivity_at` | Compute one multiplier, defaulting to full without a calculator. |
| `contains` | Check a building ID. |
| `next_available_id` | Find the first unused building ID independently of project IDs. |
| `upgrade_building` | Find a target and replace its validated definition. |
| `generate_requests` | Create nonempty building demands with typed recipients. |
| `process_allocations` | Execute each building and aggregate production/returns into `u64` totals. |

### [core/src/settlement/economy/inventory.rs](../../../core/src/settlement/economy/inventory.rs)

Private `SettlementInventoryManager` owns main/pending inventories and storage order.

| Method | Purpose |
| --- | --- |
| `new` | Create both inventories with matching capacity and default material order. |
| `set_storage_order`, `storage_order` | Validate/write or read the admission order. |
| `refresh_labor_capacity` | Reset available labour to population for this turn. |
| `process_production_results` | Admit returned inputs and new products, recording overflow by stage. |
| `commit_pending_production` | Drain the buffer into the warehouse and report transfer losses. |
| `add_in_order` | Insert a batch according to the settlement's material priority. |

### [core/src/settlement/economy/population.rs](../../../core/src/settlement/economy/population.rs)

`NeedTier` defines a population threshold and per-capita needs; `ConsumptionResult`
records satisfaction. `PopulationManager` is private; needs types are public via
the settlement root and the consumption result via `turn_report`.

| Type / method | Purpose |
| --- | --- |
| `NeedRegistry::new` | Sort need tiers by population threshold. |
| `NeedRegistry::get_active_tiers` | Iterate tiers activated by the population size. |
| `PopulationManager::new` | Store population and a shared needs registry. |
| `generate_requests` | Emit demand for the population recipient. |
| `calculate_demand` | Sum active per-capita needs and multiply by population using checked integers. |
| `process_allocation` | Evaluate satisfaction from each resource's delivered fraction. |

### [core/src/settlement/projects/mod.rs](../../../core/src/settlement/projects/mod.rs)

Re-exports `economy::cost::ProjectCost`. Contains `Project`, `ProjectResult`, `ProjectError`,
`ProjectCompletion` and `ProjectOutcome`.

| Type / method | Purpose |
| --- | --- |
| `Project::new` | Create a project with a normalized explicit cost. |
| `id`, `result`, `remaining_cost` | Read identity, intended effect and outstanding cost. |
| `as_demand_request` | Request remaining materials under a typed project recipient. |
| `resolve_allocation` | Reduce material costs and advance elapsed turn time. |
| `is_completed` | Report whether both material and time costs are paid. |

### [core/src/settlement/projects/manager.rs](../../../core/src/settlement/projects/manager.rs)

Private `ProjectManager` owns active projects. `new()` initializes the list;
`add_project()` normalizes costs and rejects duplicate IDs; `generate_requests()`
creates nonempty demands; `process_allocation()` advances every project;
`extract_completed()` removes and returns completed projects in collection order.

### [core/src/settlement/projects/lifecycle.rs](../../../core/src/settlement/projects/lifecycle.rs)

Internal project validation and effects on the same `Settlement` type.

| Method on `Settlement` | Purpose |
| --- | --- |
| `validate_project_result` | Check upgrade-target existence and supported project kinds; blueprint validity is guaranteed by its constructor. |
| `reject_invalid_projects` | Remove invalid active projects before they receive allocations. |
| `resolve_completed_projects` | Extract finished projects and collect their finalization outcomes. |
| `finalize_project` | Apply construction/upgrade effects or return an explicit failure. |
| `validate_project_site` | Delegate placement and matching reservation validation to ConstructionSpace. |
| `release_project_site` | Release only a rejected project's own construction reservation. |

### [core/src/settlement/turn/mod.rs](../../../core/src/settlement/turn/mod.rs)

| Method on `Settlement` | Purpose |
| --- | --- |
| `process_turn` | Refresh productivity, reserve trade resources, allocate local demands, execute production/projects, collect stock and return a report. |
| `prepare_turn` | Refresh productivity/labour, snapshot opening stock, reject invalid projects and reserve trade resources. |
| `allocate_turn` | Gather demands and allocate from existing stock. |
| `consume_turn` | Evaluate population allocations and shortages. |
| `progress_construction` | Pay project allocations and advance time once. |
| `produce_turn` | Execute buildings and record produced/unused materials. |
| `finish_turn` | Collect stock, report overflow, finalize projects and return trade resources. |

Contains inline unit tests of turn behavior and private aggregate invariants.

### [core/src/settlement/turn/report.rs](../../../core/src/settlement/turn/report.rs)

Contains `TurnReport`, `InventoryTurnReport`, `MaterialTotals`, `InventoryLoss`,
`InventoryLossStage`, and the `ConsumptionResult` re-export.
`InventoryTurnReport::snapshot()` sums material stock across both inventories;
`from_allocations()` records grants by consumer category; `record_production()`
records outputs and removes unused inputs from consumption;
`is_balanced()` checks material conservation separately for each resource.
Trade transfers are currently outside that accounting model; see [trading](../systems/trading.md).


## Economy

### [core/src/economy/inventory.rs](../../../core/src/economy/inventory.rs)

Contains capacity-aware `ResourceInventory`, `InventoryAddResult` and
`CapacityError`. The types are also re-exported from `simulation_core::economy`.

| Method on `ResourceInventory` | Purpose |
| --- | --- |
| `new` | Create empty stock with a material capacity limit. |
| `resources`, `amount` | Read all counters or one resource amount. |
| `capacity`, `set_capacity` | Read/change capacity, rejecting a limit below current stock. |
| `total_amount` | Sum stored materials in `u64`, excluding labour. |
| `add` | Add what fits and report accepted/rejected units. |
| `consume` | Remove exactly the requested quantity if available. |
| `consume_available` | Remove and return the entire counter for one resource. |
| `reset_labour` | Internally reset labour without warehouse capacity constraints. |
| `take_all` | Internally drain all stock for a transfer. |

### [core/src/economy/resources.rs](../../../core/src/economy/resources.rs)

Contains `ResourceType` and `ResourceType::MATERIALS`, the complete material
catalogue used to validate storage orders. No explicit methods.

### [core/src/economy/allocation.rs](../../../core/src/economy/allocation.rs)

`AllocationRecipient` distinguishes population, buildings and projects;
`DemandRequest` and `AllocationResult` carry typed demand/grant maps.
`AllocationEngine::execute()` combines demands, allocates scarce resources
proportionally with deterministic remainder handling, and withdraws material grants.

### [core/src/economy/storage_order.rs](../../../core/src/economy/storage_order.rs)

`StorageOrder` stores a validated ordered material list; `StorageOrderError`
identifies invalid lists. `default()` follows the material catalogue; `set()`
rejects missing/duplicate/unsupported entries; `get()` returns the order;
`compare()` orders two materials by their positions in the list.

### [core/src/economy/gold_inventory.rs](../../../core/src/economy/gold_inventory.rs)

`GoldInventory::new()` sets initial currency; `add()` increments it; `consume()`
spends only when sufficient; `consume_available()` drains it; `default()` starts
at zero. Currency is separate from warehouse materials.

### [core/src/ids.rs](../../../core/src/ids.rs)

`SettlementId`, `BuildingId` and `ProjectId` are distinct `u32` tuple types.
`SettlementId::fmt` formats the numeric ID without conflating it with other ID types.

## Trade and transport

### [core/src/trade/offer.rs](../../../core/src/trade/offer.rs)

Contains `TradeOffer`, `TradeDemand`, `SellTradeOffer` and `BuyTradeOffer`.

| Type / method | Purpose |
| --- | --- |
| `TradeOffer::calculate_demand` | Dispatch to the contained offer's gold/material demand. |
| `SellTradeOffer::calculate_demand` | Request the offered material quantity. |
| `update_sell_resource_amount` | Replace the funded quantity available to sell. |
| `get_amount_to_sell`, `get_minimal_price_per_unit` | Read offered stock and minimum unit price. |
| `consume_avaiable_units` | Drain unsold stock; spelling matches the current code. |
| `subtract_sold_amount` | Decrement stock after a sale. |
| `BuyTradeOffer::calculate_demand` | Request quantity multiplied by maximum unit price in gold. |
| `consume_bought_units` | Drain purchased stock for return to inventory. |
| `get_maximum_price_per_unit`, `get_remaining_amount_to_buy` | Read price ceiling and outstanding demand. |
| `add_bought_amount` | Record purchases and reduce outstanding demand. |

### [core/src/trade/book.rs](../../../core/src/trade/book.rs)

`TradeBook` owns reserved gold and offers keyed by resource.

| Method | Purpose |
| --- | --- |
| `new`, `default` | Initialize explicit offers/funds or an empty book. |
| `add_gold_amount` | Add reserved funds. |
| `update_sell_resource_amount` | Update funding for an existing sell offer. |
| `calculate_demand` | Aggregate material and currency reservations requested by offers. |
| `return_resources` | Drain purchased/unsold goods and remaining gold. |

### [core/src/trade/book/matching.rs](../../../core/src/trade/book/matching.rs)

Bilateral matching and transaction methods on the same `TradeBook` type.

| Method | Purpose |
| --- | --- |
| `trade` | Match opposing offers in two books using midpoint prices. |
| `transaction` | Execute one purchase direction, scaling quantities to funds and spending remainders. |
| `execute_single_trade` | Debit the buyer and update buyer/seller quantities. |

### [core/src/trade/allocation.rs](../../../core/src/trade/allocation.rs)

`TradeAllocationEngine::execute()` reserves materials and gold from inventories;
`execute_gold_allocation()` funds the book up to the available amount;
`execute_remained_resources()` returns remaining currency and goods to inventories.
It currently ignores the result of material admission.

### [core/src/trade/caravan.rs](../../../core/src/trade/caravan.rs)

`Caravan` carries a trade book, route, sender/receiver IDs and a `CaravanState`.
`new(map, finder, sender_id, receiver_id, start, end, book)` builds an A* route
between explicit points and returns `Option<Caravan>`; it does not read a `World`
or interpret participant IDs as province indices. `travel()` dispatches the
movement state; `trade()` matches books on arrival and starts the return journey;
`travel_to()` and `travel_back()` advance along the stored path.
See [current limitations](../systems/trading.md) before using this prototype in a simulation.

## Geography and world state

### [core/src/world/settlement_registry.rs](../../../core/src/world/settlement_registry.rs)

`SettlementRegistry` stores persistent instances in `BTreeMap<SettlementId, Settlement>`.
Only crate-internal world registration/commands can mutate the store.

| Method | Purpose |
| --- | --- |
| `default` | Initialize an empty ordered registry and automatic ID candidate zero. |
| `get` | Look up a settlement by its typed ID. |
| `iter` | Visit instances in ascending ID order. |
| `len`, `is_empty` | Read registry cardinality. |
| `get_mut` | Internally borrow an instance for validated commands or phases. |
| `next_id` | Read the next unregistered automatic ID, or None on exhaustion. |
| `register` | Reject duplicates, insert the persistent instance and skip occupied ID candidates without overflow. |



| File | Types and explicit methods |
| --- | --- |
| [world/terrain/area.rs](../../../core/src/world/terrain/area.rs) | `Area::new` stores elevation/moisture; `default` sets both to zero; `determine_biome`, `determine_elevation`, `determine_moisture` classify its values. |
| [world/terrain/biome.rs](../../../core/src/world/terrain/biome.rs) | `Biome::determine` maps elevation/moisture categories to a biome. |
| [world/terrain/elevation.rs](../../../core/src/world/terrain/elevation.rs) | `Elevation` and threshold constants; `determine` classifies elevation. |
| [world/terrain/moisture.rs](../../../core/src/world/terrain/moisture.rs) | `Moisture` and threshold constants; `determine` classifies moisture. |
| [world/map.rs](../../../core/src/world/map.rs) | `Map::new` stores buffers; `tile_index` checks coordinates/layer bounds; `province_at` resolves a province ID; `validate_building_position` enforces valid land. |
| [world/coordinates.rs](../../../core/src/world/coordinates.rs) | Compatibility re-export of `spatial::Point` and `spatial::Position`. |
| [world/province.rs](../../../core/src/world/province.rs) | `Province::new` creates an empty territory; `centroid` averages tile coordinates, returning `(0, 0)` for an empty territory. |
| [world/state.rs](../../../core/src/world/state.rs) | `World`, registries, province membership and checked tile reads; methods detailed below. |

### [core/src/world/state.rs](../../../core/src/world/state.rs)

| Method on `World` | Purpose |
| --- | --- |
| `new` | Store map/provinces and initialize empty caravans, settlements and territory. |
| `settlement_turn_parts` | Borrow a target settlement and temporary map-backed environment for global phases. |
| `add_settlement` | Register an empty settlement in exactly one existing, unoccupied province. |
| `settlement`, `settlements` | Read settlement instances; iteration follows ascending SettlementId. |
| `settlement_registry` | Borrow the central registry without public mutation access. |
| `map`, `provinces` | Read-only views of private world layers. |
| `province` | Resolve an actual province ID or return MissingProvince. |
| `initialize_settlement` | Allocate an unused ID and register an initialized settlement atomically. |
| `territory` | Read the global occupancy registry without mutation access. |
| `province_settlement` | Return the optional settlement ID in an existing province. |
| `province_at` | Resolve valid map coordinates to an existing province ID. |
| `settlement_at` | Resolve a tile to its province's optional settlement. |
| `tile_at` | Read occupancy after checking map coordinates and province. |
| `validate_settlement_position` | Require a position in the registered settlement's province. |
| `caravan_travel` | Advance every caravan's travel state. |
| `centroid` | Resolve a province by ID and return its centroid or MissingProvince. |

### [core/src/world/territory.rs](../../../core/src/world/territory.rs)

Contains `Tile`, `TileOccupancy`, settlement-qualified references and `TerritoryError`.

| Method | Purpose |
| --- | --- |
| `Tile::occupancy` | Read the structural state. |
| `occupy_empty` | Shared private vacancy check for building registration and construction reservation. |
| `TerritoryManager::tile_at` | Return an occupancy snapshot; absent entries are empty, without bounds checks. |
| `register_building` | Register a completed building only on an empty slot. |
| `reserve_construction` | Reserve an empty slot for a project. |
| `cancel_construction` | Release an exactly matching reservation. |
| `finish_construction` | Replace a matching reservation with its settlement's completed building. |
| `validate_reservation` | Check project and settlement identity without mutation. |

### [core/src/world/state/construction.rs](../../../core/src/world/state/construction.rs)

| Method on `World` | Purpose |
| --- | --- |
| `queue_construction` | Validate province membership and delegate construction through MapConstructionSpace. |
| `add_building` | Validate province membership and delegate completed-building registration. |
| `queue_upgrade` | Validate the target's province and delegate the upgrade. |
| `cancel_project` | Delegate cancellation using the shared registry, even after map edits. |
| `process_settlement_turn` | Recheck project membership and advance one settlement with shared spatial state. |
| `validate_settlement_turn` | Preflight existing project positions without mutating world state. |

### [core/src/world/state/inspection.rs](../../../core/src/world/state/inspection.rs)

`World::inspect_tile` returns `TileView`: terrain, province, optional settlement,
occupancy and borrowed building/new-construction objects. It validates coordinates
and province existence. This authoritative read does not filter agent knowledge.

## Pathfinding services

### [core/src/world/pathfinding/a_star.rs](../../../core/src/world/pathfinding/a_star.rs)

`AStar::a_star()` returns the reconstructed `Vec<Point>` route;
`travel_cost()` runs the shared search and returns its minimum directed cost
without path allocation; private `heuristic()` computes the octile lower bound.

### [core/src/world/pathfinding/dijkstra_flood.rs](../../../core/src/world/pathfinding/dijkstra_flood.rs)

`DijkstraFlood::dijkstra_flood()` returns the land points reachable within a
movement-cost budget using the shared buffer and movement rules.

### [core/src/world/pathfinding/path_finder.rs](../../../core/src/world/pathfinding/path_finder.rs)

`PathFinder` owns reusable private `Node` state. Movement constants define straight,
diagonal and slope costs.

| Method | Purpose |
| --- | --- |
| `new` | Allocate per-tile search nodes for fixed map dimensions. |
| `begin` | Start a generation and initialize its start node, clearing stale state on wrap. |
| `matches_map` | Check map dimensions and search buffer length. |
| `g_score` | Read this generation's best cost or `u32::MAX` if unreached. |
| `is_closed`, `close` | Inspect/mark an expanded node in the current generation. |
| `try_relax` | Store a route only when it improves the known cost. |
| `set` | Write current-generation cost and parent. |
| `parent` | Read a parent only if the node belongs to this generation. |
| `path_to` | Follow parents and reconstruct an ordered point route. |
| `path`, `zone_to_points` | Convert coordinate tuples into `Point` values. |
| `point_to_idx`, `idx_to_point` | Convert between grid coordinates and flat indices. |
| `land_neighbors` | Iterate the eight in-bounds, non-ocean neighbors. |
| `is_ocean` | Compare tile elevation with the ocean threshold. |
| `move_cost` | Combine step length with asymmetric uphill/downhill slope costs. |

## World generation services


### [core/src/world/generation/config.rs](../../../core/src/world/generation/config.rs)

Parameter data and six explicit `Default::default` implementations:
`WorldDimensions` sets map size, `VoronoiConfig` province count/relaxation passes,
`NoiseConfig` noise scales/range, `WarpConfig` distortion,
`SeedConfig` point/noise seeds, and `ElevationTuningFineConfig` island falloff.
These defaults are independent of runtime economy TOML.

### [core/src/world/generation/world_builder.rs](../../../core/src/world/generation/world_builder.rs)

| Method on `WorldBuilder` | Purpose |
| --- | --- |
| `generate_default` | Generate a world using all default parameter structs. |
| `new` | Allocate terrain/territory and initialize noise managers. |
| `generate` | Coordinate terrain, province relaxation and ocean cleanup. |

### [core/src/world/generation/world_builder/terrain.rs](../../../core/src/world/generation/world_builder/terrain.rs)

Terrain generation methods on the same `WorldBuilder` type.

| Method | Purpose |
| --- | --- |
| `build_elevation`, `build_moisture` | Fill terrain channels using warped noise. |
| `ocean_set` | Normalize ocean tiles to zero elevation and full moisture. |

### [core/src/world/generation/world_builder/provinces.rs](../../../core/src/world/generation/world_builder/provinces.rs)

Province generation methods on the same `WorldBuilder` type.

| Method | Purpose |
| --- | --- |
| `build_provinces` | Generate unique seeds, empty provinces and border warp coordinates. |
| `build_voronoi` | Repeat territory assignment and centroid relaxation. |
| `edge_alignment` | Turn a whole province into ocean if any of its tiles is ocean. |

### [core/src/world/generation/voronoi.rs](../../../core/src/world/generation/voronoi.rs)

`Voronoi::generate_random_points()` generates unique seeded grid
points; `apply_voronoi()` assigns tiles to the nearest warped seed and rebuilds
province territories; `calculate_centroids()` maps provinces to their centroids.

### [core/src/world/generation/noise_manager.rs](../../../core/src/world/generation/noise_manager.rs)

`NoiseManager::new()` initializes four-octave Perlin FBM using a supplied/random
seed; `get_noise_value()` samples/clamps noise;
`get_island_elevation_noise_value()` adds radial falloff and highland compression;
`get_warp_value()` distorts coordinates with noise.

## View adapter

### [adapters/src/view/map/game_screen.rs](../../../adapters/src/view/map/game_screen.rs)

`GameScreen::new()` stores textures/camera and empty overlay selections;
`run()` processes camera and mouse input, queries routes/reachable areas and draws
textures/overlays. `CameraType::index()` maps the selected layer to its texture slot.

### [adapters/src/view/map/map_camera.rs](../../../adapters/src/view/map/map_camera.rs)

`MapCamera::new()` frames the map; `begin()` activates its camera; `end()` restores
the default camera; `update()` handles WASD movement and wheel zoom;
`select_camera_type()` maps number keys to geography/elevation layers.

### [adapters/src/view/map/map_texture.rs](../../../adapters/src/view/map/map_texture.rs)

`MapTextureData` owns CPU images and GPU textures.

| Method | Purpose |
| --- | --- |
| `generate_from_world` | Render biome/elevation images and upload nearest-filtered textures. |
| `calculate_light` | Estimate terrain lighting from neighboring heights. |
| `determine_biome_color`, `determine_elevation_color` | Map terrain categories/values to colors. |
| `extract_map_borders` | Draw the outer map frame. |
| `extract_province_borders` | Draw boundaries between differing province indices. |

### [adapters/src/view/map/view_generator.rs](../../../adapters/src/view/map/view_generator.rs)

`ViewGenerator::generate()` creates textures and a map camera, then assembles a
`GameScreen`. It requires a live Macroquad context for GPU texture creation.

## Module registration, tests and supporting files

The following files only declare/export modules; they contain no explicit methods:

- [core/src/lib.rs](../../../core/src/lib.rs)
- [core/src/buildings/mod.rs](../../../core/src/buildings/mod.rs)
- [core/src/economy/mod.rs](../../../core/src/economy/mod.rs)
- [core/src/world/terrain/mod.rs](../../../core/src/world/terrain/mod.rs)
- [core/src/trade/mod.rs](../../../core/src/trade/mod.rs)
- [core/src/world/mod.rs](../../../core/src/world/mod.rs)
- [core/src/world/pathfinding/mod.rs](../../../core/src/world/pathfinding/mod.rs)
- [core/src/world/generation/mod.rs](../../../core/src/world/generation/mod.rs)
- [adapters/src/lib.rs](../../../adapters/src/lib.rs)
- [adapters/src/view/mod.rs](../../../adapters/src/view/mod.rs)
- [adapters/src/view/map/mod.rs](../../../adapters/src/view/map/mod.rs)

| File / location | Purpose |
| --- | --- |
| [core/tests/economy_validation_tests.rs](../../../core/tests/economy_validation_tests.rs) | Public-interface tests of definition validation, commands and allocation integrity. |
| [core/tests/building_productivity_tests.rs](../../../core/tests/building_productivity_tests.rs) | Public-interface tests of travel costs, configuration, output scaling and settlement isolation. |
| [core/tests/territory_construction_tests.rs](../../../core/tests/territory_construction_tests.rs) | World construction, overlap rejection, placement, upgrades and atomic failures. |
| [core/tests/project_cancellation_tests.rs](../../../core/tests/project_cancellation_tests.rs) | Cancellation, no refunds, upgrade preservation, reservation matching and identifier reuse. |
| [core/tests/simulation_tests.rs](../../../core/tests/simulation_tests.rs) | Global progression/order, decisions, local economic equivalence and headless multi-turn execution. |
| [core/tests/settlement_registry_tests.rs](../../../core/tests/settlement_registry_tests.rs) | Initialization, typed references, duplicates, stable traversal, ID allocation and persistent economic/spatial identity. |
| [core/examples/headless.rs](../../../core/examples/headless.rs) | `main` creates a farm scenario and runs a requested number of turns without graphics. |
| Inline `#[cfg(test)]` modules | Unit tests alongside the feature they verify, including turn tests in `settlement/turn/mod.rs`. |
| [.github/workflows/ci.yml](../../../.github/workflows/ci.yml) | Rust formatting, Clippy and workspace test checks. |
| [ml_brain/src/requirements.txt](../../../ml_brain/src/requirements.txt) | Empty dependency placeholder; no Python runtime implementation exists yet. |

Documentation is indexed in [docs/README.md](../../README.md). Update this map when
adding, moving or renaming an implementation file or method.

### [world/state/map_commands.rs](../../../core/src/world/state/map_commands.rs)

`World::set_tile_terrain` checks bounds and finite terrain values before replacing
one tile. `set_tile_province` checks bounds and target province, then updates the
map layer and both province tile lists. Neither exposes a mutable layer or changes
settlement/structural identities. Existing turn preflight remains responsible for
projects outside their settlement's province after a membership change.

## Map-independent boundaries and adapters

| Source | Types and methods |
| --- | --- |
| [spatial/mod.rs](../../../core/src/spatial/mod.rs) | Coordinate/occupancy values, qualified references, `TerritoryError`, and the `ConstructionSpace` contract: reserve, validate, cancel, finish, register, release rejected. |
| [economy/cost.rs](../../../core/src/economy/cost.rs) | `ProjectCost::normalize` removes zero entries; `is_fully_paid` checks materials and elapsed time. |
| [buildings/effects.rs](../../../core/src/buildings/effects.rs) | `StatType`: shared housing, inventory and trade modifier targets. |
| [buildings/productivity.rs](../../../core/src/buildings/productivity.rs) | `TravelCostSource::travel_cost` supplies a directed optional cost to the calculator. |
| [settlement/commands/command.rs](../../../core/src/settlement/commands/command.rs) | `SettlementCommand`: construction, upgrade, cancellation, building settings and storage order. |
| [world/settlement_environment.rs](../../../core/src/world/settlement_environment.rs) | `AStarTravelCosts::new/travel_cost`, `MapConstructionSpace::new` and its construction operations, `MapSettlementEnvironment::new` and delegation of both ports. |
| [world/state/commands.rs](../../../core/src/world/state/commands.rs) | `CommandError`; `World::execute_settlement_command` is the single typed command dispatcher. |

`World::settlement_turn_parts` borrows a target and temporary map adapter;
`World::settlement_mut` is a checked internal lookup for nonspatial phases.
`BuildingManager::get/get_mut` centralize instance lookup. `GameScreen::cursor_tile`
shares checked mouse-to-tile conversion between pathfinding overlays.
