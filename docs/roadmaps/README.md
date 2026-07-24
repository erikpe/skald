# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

- [Object Casts and Narrow Removal](OBJECT_CASTS_ROADMAP.md) — **planned**;
  replace scoped `narrow` with C-style checked-place casts over the existing
  polymorphic view pipeline. **OC0** is next. This roadmap must complete before
  shared-ownership implementation planning.

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
