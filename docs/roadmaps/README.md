# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

No implementation roadmap is currently in progress.

## Planned

No implementation roadmap is currently waiting to start.

## Design proposals

[Panic Runtime Trace Design Proposal](PANIC_RUNTIME_TRACE_DESIGN_PROPOSAL.md)
is a draft for review. It proposes linked native-frame trace records, inline
Linux x86-64 local-exec TLS push/pop, direct location replacement, no reserved
register, allocation-free panic rendering, and zero-cost compile-time
omission. Every decision remains open; contract promotion and an
implementation roadmap follow only after review and freezing.

The supporting
[Panic Runtime Trace Investigation](PANIC_RUNTIME_TRACE_INVESTIGATION.md)
audits current Skald and Niflheim behavior and compares the rejected and
retained implementation alternatives.

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
