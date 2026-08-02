# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Primitive string conversions](PRIMITIVE_STRING_CONVERSIONS_ROADMAP.md) —
  move explicit primitive formatting and optional parsing into
  `std::str::Str`; TXT4 (shortest round-tripping binary64 formatting) is next.
  It depends on the implemented primitive operator, optional-value, string,
  loop, and standard-I/O contracts.

## Planned

No implementation roadmaps are currently planned but not started.

## Design proposals

No design proposals are currently awaiting decisions or promotion.

## Pending discoveries

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
