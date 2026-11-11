# Archived action mask

This snapshot preserves the former action-mask model, query, tests and the
decision preparation code they used. The files are outside the Cargo workspace
and serve as reference. The active handler uses the preparation code under
`core/src/application/commands/settlement`. Agents read observations and receive
the result of each submitted action in the turn report.
