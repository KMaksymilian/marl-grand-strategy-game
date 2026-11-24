# Settlement founding and needs

[settlement_start.toml](../../../settlement_start.toml) defines the founding
profile. New settlements start with 1,000 residents, 600 displayed wood,
300 displayed stone and five copies of one turn of their starting diet.
The loader validates the whole stock against the configured warehouse capacity.

[settlement_needs.toml](../../../settlement_needs.toml) configures population
thresholds, material rates and ordered diets. Population at turn opening
selects the diet. Material needs accumulate from every unlocked tier.
Food totals 100 integer material units per resident per turn. The allocator
rounds quotas down and gives leftover units to the largest fractional
remainders in row order. After normal allocation, available food may replace
missing food one for one in `food_items` order. Only the remaining shortfall
counts toward the number of people without a full ration.

Active food decrees change the ration for the turn. Combined changes are rounded
to basis points and the multiplier has a lower bound of zero. The exact
adjusted ration determines total demand (rounded up to an integer material
unit), substitutions, the number of unfed residents and food-supported growth.
The founding stock uses the base diet.

The supplied excerpt specifies the Tier 2 grain, bread and wood rates. The
other diet shares and material rates are editable balance defaults. Beer,
tools and furniture are cumulative material needs. Monument components and
fortification components remain construction inputs.

Inventories, command quantities, reports and allocations use integer material
units. One displayed material unit equals 100 integer units. The loaders for
buildings, equipment and material limits convert displayed values once.
The per-capita need rates in TOML already use integer material units. Gold,
population, slots and turns keep their own units. Trade prices quote integer
gold per integer material unit.
