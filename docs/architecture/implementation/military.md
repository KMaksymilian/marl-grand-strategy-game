# Military

Armies of nations (gameplay specification §10 and §11). An army occupies a
whole province: its position is a province ID, and it moves from province to
province, never tile by tile.

Status: the foundations are implemented: the unit catalogue loaded from
`units.toml` and the province graph armies move on. Recruitment, upkeep,
morale, movement, battles and troop visibility build on them.

| Part | Source |
| --- | --- |
| `UnitCatalog`, `UnitDefinition`, `MilitaryRules`, `UnitTypeId` | [domain/military/unit_catalog.rs](../../../core/src/domain/military/unit_catalog.rs) |
| `ProvinceGraph` | [domain/world/province_graph.rs](../../../core/src/domain/world/province_graph.rs) |
| Content | [units.toml](../../../units.toml) |
| Tests | [unit_catalog_tests.rs](../../../core/tests/unit_catalog_tests.rs) |

## Unit catalogue

`units.toml` holds the common `[rules]` and one `[[units]]` entry per unit
type. `UnitCatalog::standard()` is the bundled file;
`UnitCatalog::load(path, resources)` reads one from disk and
`World::set_unit_catalog` installs it (the game does this at startup). Equipment
names resolve against the resource catalogue, aliases included;
`set_unit_catalog` refuses a catalogue whose equipment the world's resources
lack (`UnitEquipmentUnknown`).

| Unit (`id`) | Equipment | Attack | Defence | Movement | Advantage against | Gold upkeep |
| --- | --- | --- | --- | --- | --- | --- |
| Levy (`levy`) | none | 1 | 1 | 1 | none | 1 |
| Spearmen (`spearmen`) | 25 bronze, 25 planks | 2 | 3 | 1 | cavalry | 1 |
| Archers (`archers`) | 50 planks, 10 tools | 3 | 1 | 1 | spearmen | 1 |
| Cavalry (`cavalry`) | 25 cattle, 25 steel | 3 | 2 | 2 | archers | 2 |
| Siege engines (`siege_engines`) | 50 planks, 25 tools | 1 | 0 | 1 | walls: −25 points of the wall bonus each | 2 |

Upkeep is per soldier per turn; every unit eats 1 food per soldier.

| Rule | Value |
| --- | --- |
| Soldiers per unit | 50 |
| Conscription pool | 10% of a settlement's population (decrees may multiply it) |
| Recruitment | 2 turns |
| Advantage | ×1.5 |
| Morale | 0 to 100, starts at 100; +20 per turn with full upkeep (+30 with tier-2 food), −20 with partial upkeep |
| Battle | the loser loses 30% of its units (rounded up) and 30 morale; strength ±10% |
| Garrison | `levy` units from 5% of the population |

Loading rejects unknown fields, invalid or duplicate IDs, empty names, zero
movement, unknown or zero equipment, advantages against unknown types or the
type itself, a wall bonus reduction outside 0–1, an unknown garrison unit and
contradictory rules.

Decided for later stages: recruited soldiers leave the settlement for good, and
the dead are lost. Disbanding returns the soldiers to the settlement of the
province where the army stands; it is allowed only in an own province. Walls
are a building of the economy (the battle reads their level); the terrain
modifier is 1.0 for now.

## Province graph

`World::province_graph()` says which provinces border each other: two
provinces are neighbours when a land tile of one shares an edge with a land tile
of the other (ocean tiles and corners connect nothing). The world rebuilds the
graph whenever `set_tile_province` or `set_tile_terrain` changes a tile.

`ProvinceGraph::route(from, to, allowed)` is a breadth-first search in
ascending province ID: the shortest route in provinces through provinces for
which `allowed` holds, so equal routes are always resolved the same way.
Movement will use it with the provinces an army may enter this turn (own,
allied, or of a nation at war with its owner), recomputed every turn from the
current relations, so a route can never cross a neutral province.
