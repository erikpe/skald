# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Generic classes](GENERIC_CLASSES_ROADMAP.md) — **In progress; G9 is next.**
  Implements explicit closed generic class applications through deterministic
  semantic specialization, inferred contextual requirements, nominal
  interface bounds, and ordinary closed HIR/MIR/backend paths. The complete
  frozen syntax, template identities, structural template terms, nominal
  bounds, delayed dependent selections, contextual mechanical requirement
  inference/evaluation, and deterministic closed-specialization identity,
  caching, provenance, recursion handling, and complete ordinary closed class
  declarations and bodies, lifecycle and ownership composition, nominal bound
  enforcement, closed inheritance, per-application conformance, and ordinary
  virtual/interface dispatch are implemented. The next boundary integrates
  per-specialization statics with whole-program effects and lifecycle order.

## Planned

No implementation roadmap is currently planned behind the active roadmap.

## Design proposals

No design proposal is currently under review.

Frozen design proposals and their completed implementation roadmaps are
preserved in the [archive](../archive/README.md).

## Pending discoveries

- [Generic classes discoveries](GENERIC_CLASSES_DISCOVERIES.md) — atomic
  failed-specialization rollback should remove or rebuild every dependent
  resolved product, not only class declarations, definitions, and hierarchy.

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
