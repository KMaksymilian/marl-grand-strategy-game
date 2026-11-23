# Economic building catalog

The game loads [`buildings.toml`](../../../buildings.toml) after the resource
catalog. IDs in construction costs and recipes resolve through that catalog.
Each resulting blueprint is immutable and shared by building instances.

- `[tiers.1]`, `[tiers.2]` and `[tiers.3]` supply build time, production rate
  and staff capacity. Recipe quantities describe one complete cycle.
- `cycles_per_turn` can override the tier rate. The charcoal kiln uses the
  Tier 1 construction time and the Tier 2 production rate.
- When `biome_efficiencies` is present, omitted biomes forbid placement.
  An absent table allows all land biomes at 100% biome efficiency.
- A waterfront requires land sharing an edge with ocean. A configured mine
  selects its ore through the blueprint ID, for example `mine_iron_ore`.
  The construction site belongs to the settlement's province. The matching
  deposit may occupy any valid land tile in that province. Tile-specific
  deposits remain supported for existing programmatic definitions.

The building requests staffing and materials for one recipe. For alternatives
such as the builder's three recipes, it selects the recipe with the most
possible full cycles in the available stock before allocation. Ties use
configuration order. The allocation engine still arbitrates competing requests.
Production uses the actual grants for that recipe and returns unused inputs.
Output modifiers share the existing production calculator. They affect yield
after allocation, with the configured biome percentage applied relative to
100% before registered modifiers. A* accessibility still applies when enabled.

The settlement observation lists each blueprint's cycle rate, recipes, allowed
biomes, and placement requirements.
