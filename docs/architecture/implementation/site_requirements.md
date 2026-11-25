# Building site requirements

Construction, upgrades and production preparation use
`ConstructionSpace::evaluate_site(position, requirements)`. The map provides
current terrain facts, while the specification performs typed comparisons.
Province ownership and structural reservations are checked separately. A project
whose site stops meeting its terrain requirements is rejected before allocation.
An existing building on an unsuitable site requests no staff or inputs.

The ore-veins branch uses programmatic building registration. A scenario can
parse a site's TOML fragment with `SiteRequirements::from_toml`, then attach
the result with `BuildingDefinition::with_site_requirements` before registering
the blueprint. Programmatic coast and tile-deposit helpers add conditions to
that same specification. No full building catalog loader is needed here.

For a waterfront, use:

```toml
site_requirements = [{ fact = "tile.coastal", op = "eq", value = true }]
```

Use `{ fact = "province.deposits", op = "contains", value = "iron_ore" }` for a
mine that can be placed anywhere in a province with a valid iron deposit. Use
`tile.deposits` to require a deposit on the building's own tile. Flooded deposits
are unavailable for either requirement. The vein generator continues to use
province biomes when it rolls new deposits.

A plant can use `biome_efficiency = { grassland = 100, forest = 75 }` to allow
those two biomes. Missing biomes are forbidden when the table is present.
The resulting percentage adjusts yield after allocation, together with A*
productivity and before final rounding.

## Add a fact to a scenario

Load the following data with `World::load_site_facts_toml` before parsing
requirements. Pass `world.site_fact_types()` and the world's resource catalog
to the requirement loader.

```toml
[fact_types]
"tile.soil_fertility" = "integer"

[[tiles]]
x = 1
y = 0
[tiles.facts]
"tile.soil_fertility" = 60
```

A building can then require
`{ fact = "tile.soil_fertility", op = "gte", value = 60 }`. The supported extra
fact types are `bool`, `integer` and `id`. A missing fact on a tile fails the
condition. The loader rejects invalid values before applying any changes.

For a new fact derived from the map, declare its type in the site schema and
supply its live value in `Map::site_fact`. The settlement and its construction
interface keep the same method.
