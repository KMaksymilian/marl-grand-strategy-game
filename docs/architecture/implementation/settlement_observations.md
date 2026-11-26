# Settlement Observations

`SettlementObservation` is what one settlement agent may see. It is separate from
the authoritative state: an owned, serializable copy with no references into the
world, so an agent holding it cannot reach other settlements. Types:
[application/queries/observation.rs](../../../core/src/application/queries/observation.rs);
generation: [application/queries/observe_settlement.rs](../../../core/src/application/queries/observe_settlement.rs).

## Generation

| Method | Result |
| --- | --- |
| `Simulation::observe(id)` | Observation of one settlement now, or `WorldError::MissingSettlement`. |
| `Simulation::observe_all()` | `BTreeMap<SettlementId, SettlementObservation>` for every registered settlement. |
| `SettlementObservation::to_json()` | JSON text; the type also deserializes. |

`Simulation` keeps each settlement's `TurnReport` from the latest `step` to fill
`last_turn`. Before the first step `turn` is 0 and `last_turn` is `None`.

## Content

| Field | Meaning |
| --- | --- |
| `turn`, `settlement_id`, `province_id` | Completed global turns and identity. |
| `population`, `gold` | Population size and treasury. |
| `inventory` | `capacity`, `stored` (materials, no Labour), `stored_total`, `pending` buffer, `storage_order`. |
| `workforce` | `available` (population), `requested` (staff of active buildings), `employed` (staff received in the last allocation). |
| `needs_per_turn` | Materials the population requests each turn. |
| `buildings` | Ascending ID: type, position, active, `max_workers`, `requested_workers`, `current_workers`, `location_productivity`. |
| `projects` | Queue order: `target` (`construct` with type and position, `upgrade` with building and type), remaining materials and turns. |
| `next_project_id` | ID the next accepted construction or upgrade receives. |
| `trade` | Own `market_offers`, caravan counts and `known_markets` delivered by returning caravans. Each known market includes its settlement, province, observation turn and observed offers. |
| `last_turn` | Satisfaction, missing needs, produced, consumed by population/projects/production, lost, trade reserved/received, gold received, market trade count, caravans dispatched, unfunded caravan orders, completed and failed project IDs. |
| `territory` | Every tile of the own province in row-major order: elevation, moisture, optional `deposit`, `coastal`, occupancy and `buildable`. |
| `trade_partners` | Other settlements as `{settlement_id, province_id, nation_id, trade_allowed}`, ascending ID; `nation_id` is the public owner, `trade_allowed` is false while the two nations are at war or under an embargo, or while either settlement is under attack. |
| `building_types` | The public catalog: name, tier, `max_workers`, construction cost and turns, recipes and optional `required_deposit` and `requires_coast`. |
| `nation` | The owner's policy: `nation_id`, `tax_rate`, `capital`, `is_capital`, `active_decrees`, `at_war_with`. Every settlement belongs to a nation. See [Nations](nations.md). |

`buildable` covers province membership, land and vacancy. For a particular
blueprint, the command also checks that the tile has its `required_deposit`,
if specified. A mismatch is returned as `required_deposit_missing`.

## Information boundaries

An agent sees other settlements' IDs and provinces, plus dated market observations delivered by its own caravans:

- no stock, gold, population, buildings, projects, live offers or caravans;
- no tile outside the own province;
- a structure of another settlement inside the own province (possible after a
  border change) appears only as `foreign`, without owner or IDs;
- caravans are counted only when the observed settlement sent them.

Tests in [observation_tests.rs](../../../core/tests/observation_tests.rs) check
that two worlds differing only in another settlement's state, and in that
settlement's decisions over several turns, give identical observations.
