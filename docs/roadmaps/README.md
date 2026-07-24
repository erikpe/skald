# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

- [Object Casts and Narrow Removal](OBJECT_CASTS_ROADMAP.md) — **in progress**;
  replace scoped `narrow` with C-style checked-place casts over the existing
  polymorphic view pipeline. **OC2** is next. This roadmap must complete before
  shared-ownership implementation planning.
- [Object Casts Discoveries](OBJECT_CASTS_DISCOVERIES.md) — pending
  representation and control-effect follow-ups found while implementing the
  direct checked-place slice; both are bounded to later cast/compiler work.

The completed polymorphism profile remains the implementation baseline.
Shared-ownership language and implementation contracts are frozen in
[Shared Ownership and Heap Allocation](../language/SHARED_OWNERSHIP.md) and
the
[Shared-Ownership Compiler and Runtime Contract](../compiler/SHARED_OWNERSHIP.md),
including ordinary allocation and explicit exact-class copy allocation from a
checked cast place. No shared implementation roadmap will be created until
object casts are current. Dynamic-type-preserving cloning remains deferred.
Checked exceptions remain the later exploratory direction because they extend
cleanup to exceptional control flow. Other unscheduled language directions
and their maturity are owned by the
[status matrix](../language/STATUS.md#not-implemented).
