# Settlement happiness, demographics and income

[settlement_happiness.toml](../../../settlement_happiness.toml) defines the base
score, happiness bands, conquest and capital loss effects, starvation and gold
income. The world supplies the owner's base tax rate, war weariness, conquest
and capital-loss timers, active decrees and completed monument effects.
Happiness is clamped to 0–100. The floored score selects the band. Consumption
fulfilment remains a separate report of how much of the requested stock was used.

The turn uses one happiness snapshot before production. Below 30, the
production yield is multiplied by 0.8 through the shared modifier pipeline.
The penalty applies after input allocation. Population changes use the opening
headcount and take effect at the end of the turn. Starvation and emigration are
reported independently. Growth is limited by food left in the warehouse after
accounting for any unmet ration. Fractions of a person are rounded down for
population change.

Gold income uses the opening population and unrounded, clamped happiness.
The final amount is floored once. A population of 1,000 at happiness 50 earns
1,000 gold. The report distinguishes calculated gold from gold credited if the
treasury counter is full. The national production tax is a separate operation.

Settlements expose current happiness and the last turn's demographic and
income report. The nation's decree slots use the mean current happiness of its
settlements. The world accepts national war weariness from the system that
owns it. Changes in ownership and capital update their existing timers.
