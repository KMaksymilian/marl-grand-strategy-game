# Settlement Observations

`SettlementObservation` is what one settlement agent may see. It is separate from
the authoritative state: an owned, serializable copy with no references into the
world, so an agent holding it cannot reach other settlements. Types:
[settlement/observation.rs](../../../core/src/settlement/observation.rs);
generation: [world/state/observation.rs](../../../core/src/world/state/observation.rs).

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
| `action_mask` | Actions the settlement may submit now; see [Action mask](#action-mask). |

`buildable` uses the same placement check as the action validator
(`World::check_construction_site`), so a `Construct` on a buildable tile passes
placement validation.

## Action mask

`ActionMask` ([settlement/action_mask.rs](../../../core/src/settlement/action_mask.rs),
built in [world/state/mask.rs](../../../core/src/world/state/mask.rs)) lists legal
actions so agents can avoid invalid decisions.

| Field | Meaning |
| --- | --- |
| `available` | Every `ActionKind` (in `ActionKind::ALL` order) with whether any action of that kind is valid now. |
| `construct` | Buildable tiles, each with the building types allowed there. |
| `upgrade` | Own buildings that can be upgraded, each with allowed types (not the current type, no running upgrade). |
| `cancel_project` | Active project IDs. |
| `set_building_workers` | Each building with `max_workers`; every value `0..=max_workers` is valid. Raising staff is limited by the free population; keeping or lowering it is always allowed. |
| `storage_materials` | A valid storage order lists each exactly once. |
| `tradable_resources` | Resources allowed in offers (at most one offer each, at least one unit). |
| `caravan_receivers` | Valid caravan receivers. |
| `caravan_sell_limits`, `caravan_gold_limit` | What a caravan may sell per resource and commit to purchases (`units × price`): current stock and gold minus what the market reserves first and caravans ordered earlier this turn. A caravan must carry something (a sale or a budget above zero). |

**Same rules as the validator.** Every construct, upgrade, cancel and receiver
option is produced by running the action validator on that candidate, and the
worker range comes from the validator's own limit function
(`Batch::worker_limit`). Tests compare mask and validator in both directions for
hundreds of candidates (including off-map positions, foreign and missing IDs,
staff beyond the range and offers beyond the limits) across changing states.

**Single-action view.** The mask describes each action as the first action of a
decision. Two rules depend on the rest of the decision and stay with the
validator: at most one action per target (`Conflict`), and caravan stock and gold
limits shared by all caravans of a decision.

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
