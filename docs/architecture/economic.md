
# Resources
Resources are physically generated on the world map or crafted in specific production chains *(using materials)*. Players use them in order to satisfy escalating population *(goods)* needs or construct buildings *(building)*.

## Resource Tiers 
### Tier 1: Fundamentals
Gathered directly from map tiles. Fulfills basic population satisfaction.
- **Raw:** 
	- `Flax`, 
	- `Meat`,    
	- `Grain`, 
	- `Wood`
- **Extracted:** 
	- `IronOre`, 
	- `Coal`, 
	- `GoldOre`, 
	- `Stone`

### Tier 2: Processed Goods
Refined in mid-level settlements for advanced population needs
- **`Textiles`** ⟵ _Flax_  
- **`Beer`** ⟵ _Grain_      
- **`Iron`** ⟵ _IronOre + Coal_ 
    
### Tier 3: Advanced Manufacturing
Complex items required for high-tier population satisfaction and growth.
- **`Weapons`** ⟵ _Iron_   
- **`Tools`** ⟵ _Wood + Iron_ 

### Tier 4: Luxury Goods
Endgame commodities for maximum population satisfaction.
- **`CraftedGoods`** ⟵ _Tools + Stone_


```mermaid
stateDiagram-v2
    direction LR

    state "Tier 1" as Tier1 {
	    state Raw{
		    Flax
	        Meat
	        Grain
	        Wood
	    }
        
        state Extracted{
	        IronOre
	        Coal
	        GoldOre
	        Stone
        }
        
    }

    state "Tier 2" as Tier2 {
        Textiles
        Beer
        Iron
    }

    state "Tier 3" as Tier3 {
        Weapons
        Tools
    }
    
    state "Tier 4" as Tier4 {
        CraftedGoods
    }
    

    %% Production Flows (Transitions)
    Flax --> Textiles
    
    
    Grain --> Beer
    
    IronOre --> Iron
    Coal --> Iron
    

    Iron --> Weapons
    Wood --> Tools
    Iron --> Tools
    
    Tools --> CraftedGoods
    Stone --> CraftedGoods


```

# Population Demands

```mermaid
stateDiagram-v2
    direction TB

    state "Pop: 0 - 20" as P20
    note right of P20
        • Grain: 1.0
        • Wood: 0.5
    end note

    state "Pop: 21 - 40" as P40
    note right of P40
        • Meat: 0.5
        • Textilies: 0.5
    end note

    state "Pop: 41 - 60" as P60
    note right of P60
        • Tools: 0.1
        • Beer: 0.5
    end note

    state "Pop: 61 - 80" as P80
    note right of P80
        • SeasonedFood: 0.2
    end note

    state "Pop: 81 - 100" as P100
    note right of P100
        • Culture: 0.1
    end note

    state "Pop: 100+" as P100+
    note right of P100+
        • Jewelery: 0.1
    end note

    %% Transakcje i warunki wzrostu
    [*] --> P20
    P20 --> P40 
    P40 --> P60 
    P60 --> P80 
    P80 --> P100 
    P100 --> P100+ 
```


# Buildings

| **Name               | **Cost**                         | **Workers Capacity** | **Production**           | Other Bonuses |
| :------------------- | -------------------------------- | -------------------- | ------------------------ | ------------- |
| **Center**           | Stone 10 Wood 20                 |                      |                          |               |
| **Hause**            | Wood 10                          |                      |                          |               |
| **Warehouse**        |                                  |                      |                          |               |
| **Market**           |                                  |                      |                          |               |
| **Keep**             |                                  |                      |                          |               |
| **Lumbercamp**       | -<br>Wood 10<br>Wood 20 Tools 10 |                      | Coal or Wood             |               |
| **Field**            |                                  |                      | Flex, Grain              |               |
| **Livestock**        |                                  |                      | Meat                     |               |
| **Textile Workshop** |                                  |                      | Textilies                |               |
| **Brewery**          |                                  |                      | Beer                     |               |
| **Mine**             |                                  |                      | Coal, Gold Ore, Iron Ore |               |
| **Smelter**          |                                  |                      | Iron, Money              |               |
| **Manofacture**      |                                  |                      | Tools, Weapons, Jewelery |               |