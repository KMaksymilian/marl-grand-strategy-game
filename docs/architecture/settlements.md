# Overview
Settlements serve as the primary autonomous agents within the MARL (Multi-Agent Reinforcement Learning) environment. From an implementation perspective, they act as the central hubs where spatial physics and economic logic intersect.

## Core Design Principles

- **Independent AI Agents:** Each settlement operates as a distinct, self-contained RL agent. Its primary objective is to optimize workforce allocation and supply chains.
    
- **Sphere of Influence:** Settlements are bound to the physical World Map. Every settlement controls a specific territorial zone, which dictates its spatial boundaries for expansion and resource harvesting.
    
- **Hub & Spoke Architecture:** The internal structure of a settlement is strictly divided to force strategic trade-offs:
    
    - **The Hub (Urban Center):** A highly constrained area dedicated to advanced processing, infrastructure, and housing.
        
    - **The Spokes (Resource Nodes):** Distributed extraction sites placed within the Sphere of Influence. Goods produced here incur a logistical penalty based on their distance to the Hub.
        
- **Economic Loop:** To grow, the settlement must balance local extraction with global trade (Caravans), dynamically reacting to the escalating needs of its population across different progression tiers.

# Building Architecture & Implementation

To ensure maximum performance during MARL (Multi-Agent Reinforcement Learning) simulations, the building system strictly separates immutable data from dynamic state using the **Flyweight** and **Component** design patterns.

### 1. Definitions vs. Instances

Building's data is split into two layers:

- **`BuildingDefinition` (The Blueprint):** Stored centrally in the `BuildingFactory` registry. It contains immutable properties such as the building's `name`, `tier`, `cost`, `max_workers`.
    
- **`Building` (The Instance):** The physical entity placed on the map. It is incredibly lightweight, containing only its unique `id`, `position`, `current_workers`, and a zero-cost atomic reference (`Arc`) to its underlying blueprint.

### 2. Zoning System (Hub vs. Spoke)

Buildings are strictly classified by their `BuildingLocation` enum, which tell where the AI agent can placed them:

- **Hub:** Tightly packed in the settlement's center.
    
- **Spoke:** Distributed across the settlement's territorial grid.

### 3. Component-Based Behaviors

Instead of deep inheritance trees, building functionalities are driven by a modular `BuildingBehavior` enum. During a simulation tick (`process_tick`), a building executes its assigned behaviors based on its active workforce: 
- **Production:** Consumes specific inputs from the settlement's `ResourceInventory` and yields outputs. 
- **Stat Modifier:** A unified system that alters global settlement statistics (e.g., `HousingCapacity`, `InventoryCapacity`, `Defense`). 

### 4. Zero-Cost Upgrades

Because building logic is entirely decoupled from the instance, upgrading a building is an $O(1)$ operation. The engine simply swaps the `Arc<BuildingDefinition>` pointer to the higher-tier blueprint. 

```mermaid
classDiagram
    %% --- ENUMS ---
    class ResourceType {
        <<enumeration>>
        Wood
        Stone
        Tools
    }

    class StatType {
        <<enumeration>>
        HousingCapacity
        InventoryCapacity
        Defense
        Morale
        TradeCapacity
    }

    class BuildingLocation { 
        <<enumeration>> 
        Hub 
        Spoke
    }

    %% --- STRUCTS ---
    class ResourceAmount {
        <<struct>>
        +ResourceType res_type
        +f32 amount
    }

    class ResourceInventory {
        <<struct>>
        +HashMap~ResourceType, f32~ resources
        +f32 max_capacity
        +add(res_type: ResourceType, amount: f32)
        +consume(res_type: ResourceType, amount: f32) bool
        +has_enough(cost: Vec~ResourceAmount~) bool
    }

    %% --- BUILDINGS ---
    class BuildingBehavior {
        <<enumeration / Component>>
        +Production(inputs: Vec, outputs: Vec)
        +StatModifier(stat: StatType, base_amount: f32, per_worker_amount: f32)
    }

    %% --- (Flyweight) ---
    class BuildingDefinition {
        <<Flyweight Blueprint / Arc>>
        +String id
        +String name
        +BuildingLocation location
        +u8 tier
        +Vec~ResourceAmount~ cost
        +u32 max_workers
        +Vec~BuildingBehavior~ behaviors
    }

    %% -----
    class Building {
        <<Instance on Map>>
        +String id
        +Position position
        +Arc~BuildingDefinition~ definition
        +u32 current_workers
        +bool is_active
		+calculate_demand() HashMap~ResourceType, f32~
		+execute_production(main_inventory, ratios) HashMap~ResourceType, f32~
        +upgrade(new_definition: Arc~BuildingDefinition~)
    }

    class BuildingFactory {
        <<Registry>>
        -HashMap~String, Arc~BuildingDefinition~~ registry
        +register(def: BuildingDefinition)
        +create_building(id: String, position: Position) Option~Building~
    }

    class Settlement {
        <<Context / Hub>>
        +ResourceInventory resource_inventory
        +ResourceInventory pending_production 
        +HashMap~ResourceType, f32~ total_demand
        +Vec~Building~ hub_buildings 
        +Vec~Building~ spoke_buildings
        +process_turn()
        +get_stat_value(target_stat: StatType) f32
    }

    %% --- RELATIONS ---
    ResourceAmount --> ResourceType : relate to
    ResourceInventory --> ResourceType : contain in HashMap
    
    BuildingDefinition --> ResourceAmount : cost
    BuildingDefinition *-- BuildingBehavior : contain List
    BuildingDefinition --> BuildingLocation : placement rule
    
    BuildingBehavior --> ResourceAmount : inputs / outputs
    BuildingBehavior --> StatType : modifies
    
    BuildingFactory o-- BuildingDefinition : keep in register
    
    Building --> BuildingDefinition : ref by Arc (Zero-Cost)
    
	Settlement *-- ResourceInventory : 1 main_inventory (available NOW)
	Settlement *-- ResourceInventory : 2 pending_production (available TOMORROW)
    Settlement *-- Building : has
```
### 5. Turn procedure for building production
The production engine uses a deterministic, two-pass buffered architecture to eliminate race conditions. Buildings cannot instantly consume goods produced in the same turn. Instead, total demand is aggregated, resources are proportionally allocated, and all new products are temporarily buffered until the next turn.

```mermaid
sequenceDiagram
    autonumber
    participant S as Settlement (Manager)
    participant I as main_inventory
    participant B as Buildings (List)
    participant P as pending_production

    Note over S, B: PHASE 1: Demand Gathering (No inventory modification)
    loop For each building
        S->>B: calculate_demand()
        B-->>S: Returns: "I need X wood, Y iron"
    end
    S->>S: Sums up total demand (total_demand)

    Note over S, I: PHASE 2: Allocation Calculation
    S->>I: How many resources do we actually have?
    I-->>S: Returns inventory state
    S->>S: Calculates Allocation Ratio (min(1.0, Inventory / Demand))

    Note over S, P: PHASE 3: Execution (Consumption & Buffering)
    loop For each building
        S->>B: execute_production(allocation_ratios)
        Note right of B: Building operates at e.g., 40% efficiency
        B->>I: Consumes allocated resources
        B-->>S: Returns: "I produced Z stone"
        S->>P: Puts produced stone into Buffer
    end

    Note over S, I: PHASE 4: Accounting (End of turn)
    S->>P: Extract everything from Buffer
    P-->>S: Finished goods
    S->>I: Add products to main inventory
    Note over I: Resources are ready to use in the NEXT turn
```


# Population & Needs Management

The population system operates on a dynamic **Satisfaction System**. This architecture provides a granular, normalized metric (ranging from 0.0 to 1.0).
### 1. The Needs Registry (Data Layer)

Population demands are entirely data-driven and stored centrally. This completely decouples progression milestones from the settlement's internal logic:

- **`NeedRegistry`:** The global repository containing all demographic milestones.
    
- **`PopulationTier`:** Defines specific thresholds (`min_population`) that, once reached, unlock a new set of demands for the citizens.
    
- **`NeedDefinition`:** A blueprint specifying the required `ResourceType` and the exact `amount_per_capita` consumed per turn. 

### 2. The Population Manager (Logic Layer)

Each `Settlement` delegates its demographic state to the `PopulationManager`. The manager dynamically queries the `NeedRegistry` to retrieve the `unlocked_needs` applicable to its current population `count`.

### 3. Evaluation & Satisfaction (MARL Integration)

At the end of a turn, the `process_turn` method triggers the `evaluate_and_consume` function. This executes the core consumption loop:

1. It calculates the total volume required for each active need (Per Capita × Population Count) and attempts to withdraw it from the `ResourceInventory`.
    
2. Every active need carries an **equal proportional weight**. 
    
3. The method yields a `ConsumptionResult`, which contains the final `satisfaction_percent` and a list of `missing_resources`.
    


```mermaid
classDiagram
    %% --- Definition ---
    class NeedRegistry {
        <<Registry>>
        -Vec~PopulationTier~ tiers
        +get_active_needs(population: u32) Vec~Arc~NeedDefinition~~
    }

    class PopulationTier {
        <<struct>>
        +u32 min_population
        +Vec~Arc~NeedDefinition~~ unlocked_needs
    }

    class NeedDefinition {
        <<Flyweight Blueprint / Arc>>
        +String id
        +ResourceType resource_type
        +f32 amount_per_capita 
    }

    %% --- Settlement Logic ---
    class PopulationManager {
        <<struct>>
        +u32 count
        +f32 current_satisfaction %% Wynik od 0.0 do 1.0 (0-100%)
        +evaluate_and_consume(inv: &mut Inventory, reg: &NeedRegistry) ConsumptionResult
    }

    class ConsumptionResult {
        <<struct>>
        +f32 satisfaction_percent
        +Vec~ResourceType~ missing_resources
    }

    class Settlement {
        <<Context>>
        +Inventory inventory
        +PopulationManager demographics
        +process_turn(need_registry: &NeedRegistry)
    }

    %% --- Relations ---
    Settlement *-- PopulationManager : has
    PopulationManager ..> ConsumptionResult : generate every turn
    PopulationManager --> NeedRegistry : ask for demand
    
    NeedRegistry *-- PopulationTier : keep thresholds
    PopulationTier --> NeedDefinition : has ref
```




# Trading
## 1. Local Marketboards (Offers & Proposals)

Each `Settlement` maintains its own `MarketBoard`. AI agents explicitly post **Trade Offers**.

- **`TradeOffer`:** A structured proposal indicating the intent to Buy or Sell, the target `ResourceType`, the requested volume, and the accepted price in Gold.
    
- Agents can adjust their Marketboards every turn based on their needs.
    
- To initiate trade, a settlement dispatches a Caravan specifically targeting a known offer in another settlement.
    

## 2. Gold as a Sovereign Currency

To reflect its role as a universal medium of exchange, **Gold is mechanically separated from standard resources**.

- **Zero-Weight Exemption:** Gold is not stored in the `ResourceInventory` and does not consume `max_capacity`.
    
- **Logistics Exemption:** Transporting Gold does not consume standard caravan cargo limits.
    
- **Implementation:** It is tracked as a dedicated primitive field (`gold_balance: f32`) on both the `Settlement` and the `Caravan` entities. 
    

## 3. Caravans as Information Vectors (Delayed Observability)

In a Multi-Agent Reinforcement Learning (MARL) environment, dealing with the "Fog of War" is critical. Agents do not inherently know the prices or inventory levels of distant settlements. Instead, **Caravans act as physical data carriers**.

When a Caravan departs from a settlement (or passes through one), it captures a `MarketSnapshot`:

- **The Payload:** A lightweight data struct containing the departure settlement's current `MarketBoard`.
    
- **Information Delivery:** When the Caravan arrives at its destination, it injects this `MarketSnapshot` into the receiving Settlement's intelligence pool.
    
- **Agent Impact:** The receiving AI agent updates its Observation Space with this newly acquired data. This creates a highly realistic, delayed information network where market knowledge is only as fresh as the last arriving Caravan, forcing the AI to account for temporal uncertainty  when dispatching its own merchants.

```mermaid
classDiagram
    %% --- ENUMS ---
    class OfferType {
        <<enumeration>>
        Buy
        Sell
    }

    class ResourceType {
        <<enumeration>>
        Wood
        Stone
        Iron
        ...
    }

    %% --- TRADE & INFO DATA STRUCTURES ---
    class TradeOffer {
        <<struct>>
        +OfferType offer_type
        +ResourceType resource_type
        +f32 volume
        +f32 price_per_unit
    }

    class MarketBoard {
        <<struct>>
        +Vec~TradeOffer~ offers
        +add_offer(offer: TradeOffer)
        +remove_offer(index: usize)
    }

    class MarketSnapshot {
        <<struct / Info Vector>>
        +u32 origin_settlement_id
        +u32 recorded_turn
        +MarketBoard board_snapshot
    }

    %% --- PHYSICAL ENTITIES ---
    class ResourceInventory {
        <<struct>>
        +HashMap~ResourceType, f32~ resources
        +f32 max_capacity
    }

    class Caravan {
        <<Entity on WorldMap>>
        +u32 id
        +u32 owner_id
        +u32 target_id
        +Vec~Position~ path
        +ResourceInventory cargo
        +f32 gold_balance
        +MarketSnapshot intel
        +bool is_returning
    }

    %% --- AGENT & ENGINE ---
    class Settlement {
        <<Agent>>
        +u32 id
        +ResourceInventory resource_inventory
        +f32 gold_balance
        +MarketBoard local_market
        +HashMap~u32, MarketSnapshot~ market_intelligence
        +post_offer(offer: TradeOffer)
        +dispatch_caravan(target_id: u32, cargo: ResourceInventory, gold: f32)
        +receive_caravan(caravan: Caravan)
        +process_intel(intel: MarketSnapshot)
    }

    class WorldMap {
        <<Physics Engine>>
        +Vec~Caravan~ caravans
        +process_caravans()
    }

    %% --- RELATIONS ---
    TradeOffer --> OfferType : uses
    TradeOffer --> ResourceType : refers to
    MarketBoard *-- TradeOffer : contains list of
    MarketSnapshot *-- MarketBoard : captures state of
    
    Caravan *-- ResourceInventory : carries physical goods
    Caravan *-- MarketSnapshot : carries delayed data
    
    Settlement *-- ResourceInventory : warehouse storage
    Settlement *-- MarketBoard : manages local prices
    Settlement *-- MarketSnapshot : stores known global data
    
    WorldMap *-- Caravan : simulates movement
    Settlement ..> Caravan : instantiates (dispatches)
```
