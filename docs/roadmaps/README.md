# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Arrays implementation](ARRAYS_ROADMAP.md) — **in progress**; implement the
  frozen inline/shared array contract through verified MIR and x86-64 native
  execution. Final hardening and publication are next; array aliases,
  detached-backing anchors, copied slices, checked slice assignment, shared
  and optional-shared outer arrays, and element ownership already execute. The
  completed lifecycle, shared-ownership, explicit-dereference, and
  optional-value profiles are prerequisites; the optional-verifier
  maintainability discovery is related but not blocking.

## Planned

No implementation roadmaps are currently planned.

## Pending discoveries

- [Optional-values maintainability discoveries](OPTIONAL_VALUES_DISCOVERIES.md)
  — split the large optional MIR verifier into private structural,
  initialized-storage, and guard-analysis owners without changing semantics or
  diagnostics.

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
