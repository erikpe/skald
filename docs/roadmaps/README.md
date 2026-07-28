# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Primitive Integer Casts and Comparisons Roadmap](PRIMITIVE_INTEGER_OPERATIONS_ROADMAP.md)
  — **INT0–INT1 are complete and INT2 is next**. Exact-type `i64`, `u64`, and
  `u8` comparisons now reach verified target-independent MIR; x86-64 execution
  and explicit total two's-complement/modulo casts remain before ordinary
  standard-library string behavior resumes.
- [String Types Implementation Roadmap](STRINGS_ROADMAP.md) — **STR0–STR3
  implemented; STR4 follows the primitive-integer prerequisite**. Literal
  syntax and conditional discovery feed exact language-item validation,
  intrinsic typed `Str` production, verified target-independent descriptor
  materialization, and deterministic x86-64 immortal backing. Ordinary
  standard-library behavior is paused until matching integer comparisons and
  explicit total integer casts are implemented.

## Planned

No implementation roadmaps are currently only planned.

## Design proposals

No design proposals are currently pending confirmation.

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
