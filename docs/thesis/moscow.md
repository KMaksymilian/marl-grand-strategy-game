## 🟢 MUST 
- **Map:** The environment grid or nodes where resources and agents interact.
- **Settlements (MARL Models):** Base-level AI agents making independent economic decisions.
- **Industry & Production:** Resource gathering and crafting (the core reward driver for MARL).
- **Trading:** Logistics and exchange of surplus goods between settlements, including foreign-market snippets delivered on caravan return.

## 🟡 SHOULD 
- **Nations (HRL Models):** High-level agents that group settlements and dictate macro-goals.
- **Military:** Unit recruitment, movement, combat mechanics directed by Nations. Units, caravans and settlements observe and report troop presence in nearby provinces.
- **Diplomacy:** HRL-level interactions—declaring wars, forging alliances, and trade pacts.

## 🔵 COULD 
- **Technological Tree:** Progression system that tweaks model parameters (e.g., production multipliers).
- **Migration:** Dynamic population shifts caused by war, famine, or prosperity.
- **Player Interface:** UI for observing the simulation (becomes a MUST only if a human takes control of a Nation).
## 🔴 WON'T 
- **Culture:** Too abstract for early-stage Reinforcement Learning.
- **Social Classes:** Unnecessary complexity for the MARL models right now; keep population as a single resource.

See the [information scope decision](../decisions/0006-thesis-information-scope.md) for administrative visibility and the two observation flows.
