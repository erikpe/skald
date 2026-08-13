# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

No implementation roadmap is currently in progress.

## Planned

- [Produced exact-class method receivers](PRODUCED_EXACT_CLASS_METHOD_RECEIVERS_ROADMAP.md)
  is planned. It lets exact-class literals, constructions, and call results
  serve directly as read-only method receivers through the existing
  caller-owned produced-object temporary machinery. PER0, freezing the living
  language and compiler contract, is next. It depends only on completed object
  values, produced read-only aliases, full-expression cleanup, polymorphism,
  strings, generic classes, and `Vec<T>`; no other active roadmap blocks it.

## Design proposals

No design proposal is currently under review.

Frozen design proposals and their completed implementation roadmaps are
preserved in the [archive](../archive/README.md).

## Pending discoveries

No pending discovery is currently recorded.

## Implementation baseline

The completed initial module system, polymorphism, object-cast, and constructor
profiles remain the implementation baseline. Module behavior is specified by
the implemented
[language contract](../language/MODULES_AND_INTEROP.md#initial-module-system)
and [compiler contract](../compiler/MODULE_SYSTEM.md). Constructor overload
and explicit-copy semantics are specified in
[Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md).
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
