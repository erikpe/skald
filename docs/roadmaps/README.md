# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress, plus actionable discovery backlogs that feed them. Completed
roadmaps move to [`../archive/`](../archive/README.md).

## Planned

- [Polymorphism](POLYMORPHISM_ROADMAP.md) — planned; PM0 can now freeze the
  executable profile in the focused polymorphism design authority. The roadmap
  then extends the completed exact-class
  object model with inheritance, lifecycle composition, virtual dispatch,
  interfaces, `Obj` views, type tests, and checked narrowing. The remaining
  resolver and type-checker class orchestration follow-ups precede hierarchy
  implementation.

The current object-model dependency order is polymorphism before focused
shared-ownership work, followed by checked exceptions that extend cleanup to
exceptional control flow. Shared ownership and exceptions remain exploratory:
neither is scheduled until a focused design and roadmap make its contracts
reviewable. Other unscheduled language directions and their maturity are owned
by the [status matrix](../language/STATUS.md#not-implemented).

## Follow-up backlogs

- [Maintainability discoveries](MAINTAINABILITY_DISCOVERIES.md) — active;
  resolver class-body orchestration is followed by the corresponding
  type-checker program boundary. Both are scheduled by the polymorphism
  roadmap before hierarchy implementation; other discoveries remain here
  rather than expanding an in-progress task.
