# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress, plus actionable discovery backlogs that feed them. Completed
roadmaps move to [`../archive/`](../archive/README.md).

## Planned

- [Polymorphism](POLYMORPHISM_ROADMAP.md) — planned; PM0 is next. It extends the
  completed exact-class object model with inheritance, lifecycle composition,
  virtual dispatch, interfaces, and checked narrowing. The completed compiler
  maintainability cleanup is its baseline. Design and hierarchy validation can
  proceed now; address the lifecycle-orchestration follow-ups before PM3
  expands those owners.

## Follow-up backlogs

- [Maintainability discoveries](MAINTAINABILITY_DISCOVERIES.md) — active;
  resolver class-body orchestration is next, followed by the corresponding
  type-checker program boundary. These are medium-priority prerequisites for
  adding more lifecycle categories, not blockers for polymorphism design.
