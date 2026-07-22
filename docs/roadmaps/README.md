# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## Planned

- [Polymorphism](POLYMORPHISM_ROADMAP.md) — planned; PM0 can now freeze the
  executable profile in the focused polymorphism design authority. The roadmap
  then extends the completed exact-class
  object model with inheritance, lifecycle composition, virtual dispatch,
  interfaces, `Obj` views, type tests, and checked narrowing. Its preparatory
  resolver and type-checker class orchestration tasks are complete.

The current object-model dependency order is polymorphism before focused
shared-ownership work, followed by checked exceptions that extend cleanup to
exceptional control flow. Shared ownership and exceptions remain exploratory:
neither is scheduled until a focused design and roadmap make its contracts
reviewable. Other unscheduled language directions and their maturity are owned
by the [status matrix](../language/STATUS.md#not-implemented).
