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
| `trade` | Own `market_offers` as `OfferSpec`s, own `caravans_in_transit` and `caravans_returning`. |
| `last_turn` | Satisfaction, missing needs, produced, consumed by population/projects/production, lost, trade reserved/received, gold received, market trade count, caravans dispatched, unfunded caravan orders, completed and failed project IDs. |
| `territory` | Every tile of the own province in row-major order: elevation and moisture class, occupancy (`empty`, `own_building`, `own_construction`, `foreign`) and `buildable`. |
| `trade_partners` | Other settlements as `{settlement_id, province_id}`, ascending ID. |
| `building_types` | The public catalog: name, tier, `max_workers`, construction cost and turns, recipes. |

`buildable` uses the same placement check as the action validator
(`World::check_construction_site`), so a `Construct` on a buildable tile passes
placement validation.

## Information boundaries

An agent sees nothing of another settlement except its ID and province:

- no stock, gold, population, buildings, projects, offers or caravans;
- no tile outside the own province;
- a structure of another settlement inside the own province (possible after a
  border change) appears only as `foreign`, without owner or IDs;
- caravans are counted only when the observed settlement sent them.

Tests in [observation_tests.rs](../../../core/tests/observation_tests.rs) check
that two worlds differing only in another settlement's state, and in that
settlement's decisions over several turns, give identical observations.
