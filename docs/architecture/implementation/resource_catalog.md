# Resource Catalog

The game loads [`resources.toml`](../../../resources.toml) at startup. Each
`[[resources]]` entry defines a stable material `id`, an English display
`name`, a `tier` from 1 to 3, a `category`, and optional `aliases`. Entry order
sets the default warehouse priority, from first admitted to last.

| Category | Materials |
| --- | --- |
| Raw food | Grain, Cattle, Fish, Dates |
| Processed food | Bread, Meat, Smoked Fish, Date Syrup |
| Construction material | Wood, Stone, Clay, Limestone, Planks, Stone Bricks, Bricks |
| Metal | Coal, Iron Ore, Copper Ore, Tin Ore, Steel, Bronze |
| Defense | Fortification Components |
| Crafted good | Furniture, Tools, Monument Components |
| Beverage | Beer |

Content can refer to Coal as `coal`, `charcoal`, or `mined_coal`. A valid catalog
keeps both aliases assigned to Coal, so all three identifiers share the same
stock and trade resource. Beer is a tier 2
beverage, and Monument Components are a tier 3 crafted good. Food classes
describe materials for use in recipes and population needs. Recipes and
per-capita needs still define the quantities that the simulation uses.

## Editing and using the catalog

1. Edit `name`, `tier`, `category`, or `aliases` on an existing entry. Move the
   entire entry to change its default storage priority.
2. To add a material, add a `[[resources]]` entry to `resources.toml`.
   Use a unique lowercase snake-case ID. A new ID such as `rock_salt` is
   serialized as `RockSalt`. Keep the ID stable once data uses it.
3. Run `cargo test -p simulation_core --test resource_catalog_tests` to check
   that the catalog loads and integrates with settlement storage.

`ResourceCatalog::load` reads a catalog from disk and `resolve` resolves a
content ID or alias. The world uses its catalog to initialize settlements.
`World::new` and `Settlement::new` use the bundled catalog. An explicit
storage order must list each material in the active catalog exactly once.
`ResourceId` identifies materials dynamically. `ResourceType::Labour` represents
worker capacity in allocation and has no material catalog entry.

Use `catalog.resolve("wood")` to obtain a material for recipes, stock or trade.
The returned value is `ResourceType::Material(ResourceId)`. Catalog lookup also
accepts aliases and returns `None` for an unknown identifier. Material names are
defined in TOML and are resolved at the scenario boundary.

Content, scenario APIs, and simulation calculations share one whole material
unit. The scenario API accepts quantities as `u32`:

| Quantity | Scenario API |
| --- | --- |
| Initial stock | `Settlement::deposit` |
| Construction and upgrade costs | `ProjectCost::needed_resources` |
| Recipe inputs and outputs | `BuildingBehavior::Production` |
| Per-capita consumption | `NeedTier::needs_per_capita` |
| Trade quantity limit | `available_units` in buy and sell offers |

Population counts people. Gold counts currency, and a trade price of 4 means
4 gold per whole material unit.
See the [requirements specification](../../thesis/specyfikacja-wymagan-i-zakres-projektu.md#jednostki-surowców)
for the system-wide rule and examples.
See the [economy implementation](economy_rules.md) for turn processing and storage rules.
