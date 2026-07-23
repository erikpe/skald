# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## Discoveries

- [Polymorphism maintainability discoveries](POLYMORPHISM_DISCOVERIES.md) —
  actionable resolver and backend call-lowering structure follow-ups found
  during final hardening. They do not change the implemented language
  contract; addressing them before shared-ownership implementation would keep
  those ownership-sensitive boundaries easier to extend.

The completed polymorphism profile is now the baseline for focused
shared-ownership work. Shared ownership is the next object-model direction;
checked exceptions follow because they extend cleanup to exceptional control
flow. Both remain exploratory until focused designs and roadmaps make their
contracts reviewable. Other unscheduled language directions and their maturity
are owned by the [status matrix](../language/STATUS.md#not-implemented).
