# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Golden stream matcher lists](GOLDEN_STREAM_MATCHER_LISTS_ROADMAP.md) —
  in progress; the independent matcher engine is complete and the symmetric
  schema-2 stream contract is next. No active roadmap dependency.

## Planned

No implementation roadmap is currently planned but not started.

## Design proposals

No design proposals are currently awaiting decisions or promotion.

## Pending discoveries

- [Primitive string conversion maintainability](PRIMITIVE_STRING_CONVERSIONS_DISCOVERIES.md)
  — centralize canonical standard-library test fixture closures while
  preserving explicit module overrides and source-order determinism, and fix
  produced optional shared-array result unwrapping through verified MIR.
- [Standard I/O maintainability](STANDARD_IO_DISCOVERIES.md) — extract
  assembly-only native probe builders from the backend behavior suite in a
  future focused test-organization cleanup.

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
