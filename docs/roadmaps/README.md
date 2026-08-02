# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Standard I/O](STANDARD_IO_ROADMAP.md) — **in progress**. Add four
  whole-stream `std::io` functions backed by private `u8[]` intrinsics and a
  small versioned handle/read/write/open/close runtime boundary while retaining
  the bootstrap observability helpers. IO0 froze the contracts and IO1
  published the independently tested runtime ABI, IO2 implemented the closed
  private-intrinsic registry and typed I/O HIR, IO3 added verified
  target-independent I/O MIR, and IO4 connected it to the version-7 runtime on
  x86-64. IO5 implemented exact standard-stream writes; IO6 whole-input reads
  are next.

## Planned

No implementation roadmaps are currently planned but not started.

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
