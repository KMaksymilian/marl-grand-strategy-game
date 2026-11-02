
Status: implemented. Summary updated October 29, 2026.

## Context

A settlement distributes limited resources between population, buildings and projects.
The rules must keep quantities consistent and results repeatable.

## Decisions

1. **One integer resource unit.** Stock, per-capita needs, recipes and project costs
   use the same whole units, avoiding conversions and inconsistent scales.
2. **Separate identities.** Settlements, buildings and projects have typed IDs.
   Allocation also distinguishes population, buildings and projects, preventing collisions.
3. **Proportional allocation.** Resources are shared according to demand. Remaining
   whole units go to the largest remainders; ties use a fixed recipient order.
   Materials are withdrawn before grants are issued, preventing double spending.
4. **Renewable labour.** Population supplies a new work budget each turn.
   Labour does not occupy material storage.
5. **Predictable production.** Recipes run in definition order using remaining inputs.
   Input consumption rounds up and output rounds down. Products become available
   to allocation in the next turn; construction finishes after production.
6. **Explicit storage order.** Each settlement specifies an ordered material list,
   without numeric priority weights. Returns are admitted before new products.
   Existing stock stays; overflow is discarded and reported.
7. **Validated projects.** Invalid projects are rejected before further spending.
   Cancellation releases construction reservations but preserves buildings when
   canceling upgrades. Paid materials are not refunded. Project time currently
   advances even without material grants.


## Still to decide

- Whether allocation ties should rotate between turns.
- Whether project time should advance only after funding or actual work.

## Details

- [Economy](../architecture/implementation/economy_rules.md)
- [Settlements](../architecture/implementation/settlement_commands.md)
- [Global simulation](../architecture/implementation/simulation_commands.md)
