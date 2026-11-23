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

## Special buildings

The `[[special_buildings]]` entries use the same catalog and construction
workflow. They configure costs, build times, passive effects, shared placement
caps, and any required previous blueprint. Special buildings use tier 0 in the
catalog and require no staff. The default build time is three turns, with five
and eight turns for the second and third wall levels.

Warehouse bonuses increase shared material capacity and stack. Demolition
removes the bonus immediately. Existing stock is retained if the settlement
temporarily exceeds its reduced capacity. The settlement decision
`demolish` removes an owned building without a refund.

The [game configuration](../../../configuration.toml) sets initial settlement
trade capacity. Each market adds to all four limits. Buy and sell quantities
count toward open units. Queued and travelling caravans share the slot and cargo
limits. Demolition leaves caravans in transit intact. Offers that exceed the
new cap fund only eligible units in resource order. Further markets can restore
funding for the published quantities.

The four-monument cap counts completed monuments and pending construction.
Monuments expose a permanent happiness bonus while standing. Wall upgrades
replace the preceding level's defense bonus. The province defense query
returns that bonus for the defending garrison and army. The settlement
observation also exposes passive effects and current trade limits. Happiness
totals and battle resolution can consume these values when those systems are
implemented.
