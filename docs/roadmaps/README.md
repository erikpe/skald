# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Panic and Unrecoverable Failure Reporting](PANIC_ROADMAP.md) — **in
  progress**. P0 froze the source, intrinsic, reporting ABI, and hard-trap
  contracts. P1 implemented the version-6 runtime reporter and exact native
  stderr expectations without adding compiler-generated reporter calls. P2
  implemented the canonical intrinsic declaration, validation, stable
  identity, and temporary pre-HIR call diagnostic. P3 implemented executable
  source panic from type checking through exact native reporting. P4 routed
  static MIR failures and valid host-allocation exhaustion through the same
  reporter, and P5 separated legal ownership overflow from invalid-state
  traps. P6 is next.

## Planned

No implementation roadmaps are currently planned.

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
