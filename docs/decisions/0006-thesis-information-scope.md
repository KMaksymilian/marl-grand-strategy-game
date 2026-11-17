# 0006 - Information Scope for the Thesis

Status: accepted. Market observations implemented. Troop presence pending.

## Context

The thesis schedule limits the information network to two kinds of
observations: troop presence and foreign-market conditions.

## Decisions

1. **Resolve administration immediately.** Issued decrees and orders,
   settlement reports of decree execution or shortages, and tax events become
   global knowledge as soon as the simulation processes them. An order becomes
   known when issued. Its effects follow the normal execution rules.
2. **Observe and report troop presence.** Military units, caravans and
   settlements observe whether troops are present in nearby provinces within
   their visibility area. Each observation records the source, province and
   observation turn. Units and settlements report to their owners. Caravans
   deliver the observations to their sending settlement on return.
3. **Learn about foreign markets through caravans.** The market observation
   strategy captures visible offers and prices before trading, with the market,
   province and observation turn. Caravans deliver the snapshot to the sending
   settlement on return, including after a visit with zero trades. The
   settlement's trade manager keeps the latest observation per market.
4. **Use a common snapshot for each kind of information.** Each observation
   strategy reads the data relevant to its subject and produces an
   `InformationSnapshot`. A caravan can carry multiple snapshots collected
   during a journey. The same transport works for market data and future
   observations such as troop presence.

## Still to decide

Set the visibility area and observation timing for each kind of observer. Other
observation types and a broader information network are deferred beyond the
thesis scope.

## Details

- [Thesis MoSCoW priorities](../thesis/moscow.md)
- [Trading and caravans](../architecture/systems/trading.md)
