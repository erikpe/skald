# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Integer bitwise operators and checked shifts](INTEGER_BITWISE_AND_SHIFT_OPERATORS_ROADMAP.md)
  — **In progress; BW0 through BW3 are complete and BW4 is next.** Implements exact-width
  integer `~`, `&`, `|`, and `^` first, then eager `<<` and `>>` with verified
  `u64` count checks and the existing common panic reporter. It depends on the
  completed primitive integer, eager boolean, and short-circuit boolean
  operator work plus the frozen primitive operator profile.

## Planned

No implementation roadmaps are currently planned.

## Design proposals

No design proposals are currently awaiting decisions or promotion.

## Pending discoveries

No implementation discoveries are currently pending.

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
