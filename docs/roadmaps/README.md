# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress, plus actionable discovery backlogs that feed them. Completed
roadmaps move to [`../archive/`](../archive/README.md).

## Planned

- [Documentation overhaul](DOCUMENTATION_OVERHAUL_ROADMAP.md) — planned;
  the functions and control-flow rewrite is next. The broad language overview,
  support/maturity matrix, exact grammar, and type/value/expression semantics
  are established alongside the documentation authority, migration inventory,
  discrepancy ownership, and local link/index checking. The roadmap replaces
  the remaining draft and architecture monoliths with verified, focused
  language, compiler, runtime, and development documentation.
- [Polymorphism](POLYMORPHISM_ROADMAP.md) — planned; executable profile design
  follows the documentation overhaul's language foundation and focused
  polymorphism design destination. It then extends the completed exact-class
  object model with inheritance, lifecycle composition, virtual dispatch,
  interfaces, `Obj` views, type tests, and checked narrowing. The remaining
  resolver and type-checker class orchestration follow-ups precede hierarchy
  implementation.

## Follow-up backlogs

- [Documentation overhaul discoveries](DOCUMENTATION_OVERHAUL_DISCOVERIES.md) —
  active; currently owns the legacy draft's overstated authority claim and a
  duplicate polymorphism-roadmap test line found during migration. The grammar
  cleanup has been resolved.
- [Maintainability discoveries](MAINTAINABILITY_DISCOVERIES.md) — active;
  resolver class-body orchestration is followed by the corresponding
  type-checker program boundary. Both are scheduled by the polymorphism
  roadmap before hierarchy implementation; other discoveries remain here
  rather than expanding an in-progress task.

## Supporting migration records

- [Documentation overhaul migration inventory](DOCUMENTATION_OVERHAUL_INVENTORY.md) —
  active; maps every current living-document heading and incoming legacy
  reference to its intended focused authority.
