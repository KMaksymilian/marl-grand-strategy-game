```mermaid
classDiagram
    direction TB

    namespace NationAggregate {
        class Nation["<<Aggregate Root>>\nNation"]
        class Diplomacy["DiplomaticRelation"]
        class Treasury["NationalTreasury"]
    }
    Nation *-- Diplomacy : encapsulates
    Nation *-- Treasury : encapsulates

    namespace SettlementAggregate {
        class Settlement["<<Aggregate Root>>\nSettlement"]
        class Warehouse["LocalWarehouse - Local Resources"]
        class Production["ProductionQueue"]
        class Buildings["Buildings"]
        class TradeContract["TradeContract\n(Buy/Sell Orders)"]
    }
    Settlement *-- Warehouse : encapsulates
    Settlement *-- Production : encapsulates
    Settlement *-- Buildings : encapsulates
    Settlement *-- TradeContract : encapsulates

    namespace ArmyAggregate {
        class Army["<<Aggregate Root>>\nArmyGroup"]
        class CombatStats["CombatStrength"]
    }
    Army *-- CombatStats : encapsulates

    namespace EnvironmentAggregate {
        class Map["<<Aggregate Root>>\nWorldMap"]
        class Node["Node"]
    }
    Map *-- Node : encapsulates

    %% Inter-Aggregate Communication (By ID, not direct reference)
    Nation ..> Settlement : Modifies via [SettlementID] (HRL Policy & Political Decisions)
    Nation ..> Army : Commands via [ArmyID]
    Settlement ..> Army : Spawns / Supplies via [ArmyID]
    Army ..> Map : Reads Topology via [NodeID]
    
    
    %% Inter-Aggregate Communication 
    Settlement ..> Settlement : Trades with [TargetID] 
    Settlement ..> Map : Queries optimal path & Commands road upgrades via [NodeIDs]
```

- **Nation Aggregate (Macro-Controller):** Represents the HRL model or the Player. It manages the global treasury and diplomatic relations. It steers the economy by imposing policies on Settlements and issues strategic commands to Armies.

- **Settlement Aggregate (Economic Engine):** The core MARL agent. It is entirely responsible for its internal micro-economy: managing the local warehouse, production queues, building construction, and executing trade contracts. It also handles the spawning and supplying of Armies.
    
- **Army Aggregate (Military Unit):** An independent military entity encapsulating combat strength and status. While it is spawned and supplied by a Settlement, its movement and combat orders are strictly dictated by the Nation.
    
- **Environment Aggregate (World Map):** The spatial representation of the game world. It consists of nodes (hexes/regions). It is primarily queried by Settlements for optimal trade pathfinding and by Armies for movement topology.