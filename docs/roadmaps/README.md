# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress, plus actionable discovery backlogs that feed them. Completed
roadmaps move to [`../archive/`](../archive/README.md).

## Planned

- [Documentation overhaul](DOCUMENTATION_OVERHAUL_ROADMAP.md) — planned;
  legacy entry-point removal is next. The broad language overview,
  support/maturity matrix, exact grammar, type/value/expression,
  function/control-flow, and exact-class initialization/place semantics are
  established alongside exact-class copying, assignment, materialization, and
  deterministic lifetime. Exact-class alias parameters and their ownership
  boundary now also have a focused authority. Exploratory polymorphism now has
  one focused design authority with explicit open profile choices. The
  single-file namespace, entry point, primitive foreign interoperation, and
  open module boundary now have one focused authority. Compile-time rejection,
  current runtime-failure boundaries, and future exceptional cleanup also have
  a focused authority; premature optional, array, string, iteration, and
  exception sketches have been pruned. Durable compiler responsibilities,
  phase products, target-independent IR, verification, dumps, extension
  policy, the repository-internal crate API, the backend/target contract, the
  public runtime ABI, driver/artifact behavior, contributor workflow, testing,
  and debugging now have focused authorities.
  Documentation policy, the migration inventory, discrepancy ownership, and
  local link/index checking support removal of the superseded monoliths and
  compatibility entry points.
- [Polymorphism](POLYMORPHISM_ROADMAP.md) — planned; PM0 can now freeze the
  executable profile in the focused polymorphism design authority. The roadmap
  then extends the completed exact-class
  object model with inheritance, lifecycle composition, virtual dispatch,
  interfaces, `Obj` views, type tests, and checked narrowing. The remaining
  resolver and type-checker class orchestration follow-ups precede hierarchy
  implementation.

## Follow-up backlogs

- [Documentation overhaul discoveries](DOCUMENTATION_OVERHAUL_DISCOVERIES.md) —
  active; the superseded draft still duplicates backend layout and receiver
  ABI details scheduled for removal with the monolith. The grammar,
  legacy-draft authority, stale lifecycle, alias/ownership maturity, and
  polymorphism-roadmap cleanup findings have been resolved.
- [Maintainability discoveries](MAINTAINABILITY_DISCOVERIES.md) — active;
  resolver class-body orchestration is followed by the corresponding
  type-checker program boundary. Both are scheduled by the polymorphism
  roadmap before hierarchy implementation; other discoveries remain here
  rather than expanding an in-progress task.

## Supporting migration records

- [Documentation overhaul migration inventory](DOCUMENTATION_OVERHAUL_INVENTORY.md) —
  active; maps every current living-document heading and incoming legacy
  reference to its intended focused authority.
