# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Initial Module-System Implementation Roadmap](MODULE_SYSTEM_ROADMAP.md) —
  implements the frozen whole-program language and compiler contracts across
  source syntax, anonymous providers, filesystem loading, deterministic
  identities, visibility and imports, external linkage, the driver, and
  end-to-end tests. Status: in progress; MS7 is next. Material
  dependencies: the frozen
  [language contract](../language/MODULES_AND_INTEROP.md#frozen-initial-module-system),
  frozen [compiler contract](../compiler/MODULE_SYSTEM.md), and the existing
  flat whole-program phase pipeline.

## Planned

No implementation roadmaps are currently planned.

## Pending discoveries

No maintainability discoveries are currently pending.

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
