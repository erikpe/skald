# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Short-Circuit Boolean Expressions Roadmap](SHORT_CIRCUIT_BOOLEAN_EXPRESSIONS_ROADMAP.md)
  — in progress; implement exact-`bool` `&&` and `||` with skipped effects,
  path-dependent lifetime and cleanup, arbitrary valid operands, and every
  current expression consumer. Verified MIR path conditions, conditional
  full-expression cleanup planning, and internal structured logical lowering
  are complete; the next task preserves path-dependent object and
  optional-object lifetimes. The roadmap depends on the completed eager
  boolean, control-flow, lifecycle, optional,
  shared-ownership, array, cast, and panic foundations.

## Planned

No implementation roadmaps are currently planned.

## Design proposals

No design proposals are currently awaiting decisions or promotion.

## Pending discoveries

No maintainability discoveries are currently pending.

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
