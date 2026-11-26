# Military

Armies of nations (gameplay specification §10 and §11). An army occupies a
whole province: its position is a province ID, and it moves from province to
province, never tile by tile.

Status: the unit catalogue loaded from `units.toml`, the province graph,
armies, recruitment, upkeep, morale, desertion, grouping, disbanding, marches,
sieges, battles, retreats and conquest are implemented. Troop visibility
builds on them.

| Part | Source |
| --- | --- |
| `UnitCatalog`, `UnitDefinition`, `MilitaryRules`, `UnitTypeId` | [domain/military/unit_catalog.rs](../../../core/src/domain/military/unit_catalog.rs) |
| `ProvinceGraph` | [domain/world/province_graph.rs](../../../core/src/domain/world/province_graph.rs) |
| Content | [units.toml](../../../units.toml) |
| `Unit`, `Army`, `Recruitment`, `ArmyRegistry` | [domain/military/army.rs](../../../core/src/domain/military/army.rs) |
| `MilitaryTurnReport`, `ArmyUpkeepReport` | [domain/military/report.rs](../../../core/src/domain/military/report.rs) |
| Recruitment, grouping, upkeep in World; `Conscription` | [domain/world/state/military.rs](../../../core/src/domain/world/state/military.rs) |
| Battle arithmetic (strength, advantage, walls, losses) | [domain/military/battle.rs](../../../core/src/domain/military/battle.rs) |
| Access, marches, battles, conquest, sieges, garrisons in World | [domain/world/state/war.rs](../../../core/src/domain/world/state/war.rs) |
| Tests | [unit_catalog_tests.rs](../../../core/tests/unit_catalog_tests.rs), [army_tests.rs](../../../core/tests/army_tests.rs), [war_tests.rs](../../../core/tests/war_tests.rs) |

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

Decided for later stages: walls are a building of the economy (the battle
reads their level); the terrain modifier is 1.0 for now.

## Armies, recruitment and upkeep (§10)

A **unit** (`Unit`) is `soldiers_per_unit` people of one type with its living
`soldiers`, its `morale` and the settlement it was `raised_in`. An **army**
(`Army`) is a group of one nation's units in one province; its movement is
the speed of its slowest unit (`World::army_movement`). `ArmyRegistry` in World
keeps every army and the recruitments in progress (`Recruitment`).

| Order (`NationAction`) | Effect | Errors |
| --- | --- | --- |
| `recruit_units { settlement_id, unit, units }` | `units_recruited` with `soldiers`, `ready_in_turns` and the `army_id` when ready at once | `unknown_unit_type`, `empty_recruitment`, `unknown_settlement`, `settlement_not_owned`, `conscription_pool_exceeded` (`requested`, `available`), `insufficient_stock` (equipment), `army_ids_exhausted` |
| `disband_army { army_id }` | `army_disbanded` with the `settlement_id` and `soldiers` | `unknown_army`, `army_not_owned`, `not_own_province` |
| `merge_armies { army_id, into }` | `armies_merged` | `same_army`, `unknown_army`, `army_not_owned`, `armies_apart` |
| `split_army { army_id, units }` | `army_split` with the `new_army_id` | `unknown_army`, `army_not_owned`, `invalid_unit_selection` (empty, repeated, outside the army or all of it) |

- **Recruitment (§10.2).** The people leave the settlement's population and
  the equipment the national stock when the order runs, authoritatively: fewer
  people work from the next turn. The units join the nation's army in the
  settlement's province (or a new one) at the end of the turn in which
  `recruitment_turns` run out: ordered in turn T with 2 turns, they act from
  turn T+2. With Mobilization (`instant_recruitment`) they join at once. Units
  of a settlement lost meanwhile join at the capital.
- **Conscription pool.** At most `conscription_share` (10%, ×2 with
  Mobilization) of a settlement's people serve at once, counted from its
  population plus the soldiers raised there who serve or are being recruited
  (`World::conscription`: `serving`, `limit`, `available`).
- **Upkeep (§10.3).** Right after the decree upkeep every nation feeds and
  pays all its armies from the national stock and treasury: `food_upkeep`
  food and `gold_upkeep` gold per soldier (×1.25 with Discipline, rounded up).
  Tier-2 (processed) food is taken first, then tier 1. What is available is
  shared equally, so all units of a nation get the same share and the same
  morale change: +20 with full upkeep, +30 when all the food was tier 2,
  otherwise −20 (×0.5 with Discipline), between 0 and 100. A unit at morale 0
  deserts and is gone; an army without units disappears.
- **Disbanding.** Only in an own province: the living soldiers join that
  province's settlement, not the one they came from.
- **Extinction.** A nation that leaves the game takes its armies and
  recruitments with it.

The nation observation shows `military`: own armies with their units, the
recruitments in progress, the conscription pool of every own settlement and
the food and gold upkeep per turn. The nation report shows `military`: the
upkeep paid, the morale change, deserted units and recruits that joined.

## For the economy

| What | Where | For |
| --- | --- | --- |
| Recruited people leave the settlement, disbanded soldiers join one | `Settlement::remove_population`, `add_population` (`ECONOMY` note) | fewer workers from the next turn; workers above the population are the settlement's problem |
| Armies eat from the national stock, not from the settlement | `World::pay_army_upkeep` | soldiers are not part of any settlement's needs |
| A settlement under attack produces half (§11.2) | `World::is_under_attack(settlement)` | apply −50% production, e.g. with a production modifier; tax and trade are already blocked |
| Walls (§11.4) | `World::wall_bonus` (`ECONOMY` note) | return the level's bonus (0.25 / 0.5 / 1.0), e.g. from `province_fortification_defense_basis_points` |
| Walls lose a level on conquest (§11.5) | `World::conquer` in `war.rs` (`ECONOMY` note) | call the economy's downgrade of the walls |
| Fallen garrison soldiers leave the population | `Settlement::remove_population` | fewer people and workers |
| War weariness | `RelationObservation::since_turn`, `Relation::War { aggressor }` | computed by the economy from the relations |

## War on the map (§11)

Every turn, after the armies are fed (`World::resolve_war`):

1. **Withdrawals.** An army standing where it may no longer be (after a peace
   or a broken alliance) returns to the nearest own province (`withdrawals`
   in the report); without one it is lost.
2. **Marches (§11.1).** `move_army { army_id, province_id }` sets a
   destination; the army enters one province per movement point (the speed of
   its slowest unit) along the shortest route through provinces it may enter:
   its own, its allies', those of nations at war with it, and provinces without
   a settlement. The route is recomputed at every step, so a new peace or war
   changes it. The army stops on arrival, when no route is left (`no_route`)
   and when it enters an enemy province: that is an attack on its settlement
   (`attacking`). Armies of warring nations in a third nation's province pass
   each other.
3. **Battles (§11.3).** One battle in every province (ascending ID) where an
   army of a nation at war with the settlement's owner stands. The attackers
   are all such armies; the defenders are the garrison and the armies of the
   owner and its allies there. Strength of a side:
   `Σ stat × soldiers / 50 × advantage × terrain × morale / 100`, for the
   defenders also `× walls`, then `× variance` (±10%, `set_battle_seed`).
   - Advantage: ×1.5 weighted by the share of enemy soldiers of the types the
     unit beats (spearmen > cavalry > archers > spearmen).
   - Terrain: `World::terrain_modifier`, 1.0 for now.
   - Walls (§11.4): `1 + wall bonus − 0.25 per attacking siege engine unit`,
     at least 1; the bonus comes from the walls (economy) and the owner's
     National Fortification (+10 points).
   - The higher strength wins; a tie goes to the defenders. Every unit of the
     loser loses 30% of its soldiers (rounded up) and 30 morale (×0.5 with
     Discipline); every unit of the winner loses `30% × loser / winner`
     (rounded down). Units without soldiers are gone; attacking units at
     morale 0 desert.
   - Beaten attackers retreat to the province they came from if they may
     stand there; otherwise they are destroyed. The attack ends.
   - Beaten defenders stay: the battle goes on next turn.
4. **Conquest (§11.5).** The settlement falls when its defenders have no
   soldiers or no morale left, or when there are no defenders at all (then
   without a battle). It passes, with its province, to the attacking nation
   with the most soldiers there; the defending armies there are destroyed.
   `transfer_settlement` applies the conquest penalty, the half tax, the
   capital loss and the extinction of a nation left without settlements.
5. **Sieges (§11.2).** A settlement is under attack while an army of a nation
   at war with its owner stands in its province: it pays no tax and does not
   trade (`settlement_under_attack` for caravan orders, `trade_allowed` false,
   caravans to or from it do not trade).

**Garrison (§11.4).** Levy from 5% of the settlement's people, without upkeep
and outside the conscription pool (`World::garrison`). The fallen leave the
population. It refills to 5% at the end of every turn in which the
settlement is not under attack; a conquered settlement starts empty.

| Order | Effect | Errors |
| --- | --- | --- |
| `move_army { army_id, province_id }` | `army_march_ordered` with the `route` as of now (empty: the army stays) | `unknown_army`, `army_not_owned`, `unknown_province`, `no_route` |

The nation report's `military` adds `withdrawals`, `marches` and `battles`
(every battle the nation took part in, as attacker, defender or ally:
strengths, wall multiplier, winner, losses, retreated and destroyed armies,
`conquered_by`). The nation observation shows each army's `destination` and
each own settlement's `garrison`.



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
