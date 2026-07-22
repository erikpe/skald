# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress, plus actionable discovery backlogs that feed them. Completed
roadmaps move to [`../archive/`](../archive/README.md).

## Planned

- [Polymorphism](POLYMORPHISM_ROADMAP.md) — planned; executable profile design
  is next. It extends the completed exact-class object model with inheritance,
  lifecycle composition, virtual dispatch, interfaces, `Obj` views, type tests,
  and checked narrowing. The compiler-maintainability cleanup is its baseline;
  the remaining resolver and type-checker class orchestration follow-ups are
  ordered prerequisites to hierarchy implementation.

## Follow-up backlogs

- [Maintainability discoveries](MAINTAINABILITY_DISCOVERIES.md) — active;
  resolver class-body orchestration is followed by the corresponding
  type-checker program boundary. Both are scheduled by the polymorphism
  roadmap before hierarchy implementation; other discoveries remain here
  rather than expanding an in-progress task.
