# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Polymorphism](POLYMORPHISM_ROADMAP.md) — in progress; its executable profile
  is frozen, and base initialization and lifecycle composition are next. The
  roadmap extends the completed exact-class object model with inheritance,
  lifecycle composition, virtual dispatch, interfaces, `Obj` views, type
  tests, and checked narrowing. Direct-base syntax, identity, hierarchy
  validation, and inherited lookup are complete; executable inheritance
  remains blocked before HIR.

The current object-model dependency order is polymorphism before focused
shared-ownership work, followed by checked exceptions that extend cleanup to
exceptional control flow. Shared ownership and exceptions remain exploratory:
neither is scheduled until a focused design and roadmap make its contracts
reviewable. Other unscheduled language directions and their maturity are owned
by the [status matrix](../language/STATUS.md#not-implemented).
