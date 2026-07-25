# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## Planned

- [Explicit Shared Dereference](EXPLICIT_SHARED_DEREFERENCE_ROADMAP.md) —
  **in progress**; the existing handle-to-place semantic boundary is
  centralized and explicit syntax is next. The roadmap makes `*owner` and
  `owner->member` the required source boundary for shared-pointee access while
  preserving current ownership, hidden-anchor, MIR, backend, runtime, and ABI
  behavior. It depends only on
  the completed shared-ownership, object-cast, polymorphism, constructor,
  alias, and deterministic-cleanup profiles.

## Implementation baseline

The completed polymorphism, object-cast, and constructor profiles remain the
implementation baseline. Constructor overload and explicit-copy semantics are
specified in [Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md).
Shared-ownership language and implementation contracts are current in
[Shared Ownership and Heap Allocation](../language/SHARED_OWNERSHIP.md) and
the
[Shared-Ownership Compiler and Runtime Contract](../compiler/SHARED_OWNERSHIP.md),
including ordinary allocation and explicit exact-class copy allocation from a
target-directed checked source. Dynamic-type-preserving cloning remains
deferred.
Checked exceptions remain the later exploratory direction because they extend
cleanup to exceptional control flow. Other unscheduled language directions
and their maturity are owned by the
[status matrix](../language/STATUS.md#not-implemented).
