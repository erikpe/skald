# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress, plus actionable discovery backlogs that feed them. Completed
roadmaps move to [`../archive/`](../archive/README.md).

## Planned

- [Documentation overhaul](DOCUMENTATION_OVERHAUL_ROADMAP.md) — planned;
  the language overview and feature-status authority are next. Documentation
  authority, migration inventory, discrepancy ownership, and local link/index
  checking are established. The roadmap replaces the draft, grammar, and
  architecture monoliths with verified, focused language, compiler, runtime,
  and development documentation.
- [Polymorphism](POLYMORPHISM_ROADMAP.md) — planned; executable profile design
  follows the documentation overhaul's language foundation and focused
  polymorphism design destination. It then extends the completed exact-class
  object model with inheritance, lifecycle composition, virtual dispatch,
  interfaces, `Obj` views, type tests, and checked narrowing. The remaining
  resolver and type-checker class orchestration follow-ups precede hierarchy
  implementation.

## Follow-up backlogs

- [Documentation overhaul discoveries](DOCUMENTATION_OVERHAUL_DISCOVERIES.md) —
  active and currently empty; owns contradictions, behavior defects,
  unresolved choices, and unrelated cleanup found during the migration.
- [Maintainability discoveries](MAINTAINABILITY_DISCOVERIES.md) — active;
  resolver class-body orchestration is followed by the corresponding
  type-checker program boundary. Both are scheduled by the polymorphism
  roadmap before hierarchy implementation; other discoveries remain here
  rather than expanding an in-progress task.

## Supporting migration records

- [Documentation overhaul migration inventory](DOCUMENTATION_OVERHAUL_INVENTORY.md) —
  active; maps every current living-document heading and incoming legacy
  reference to its intended focused authority.
