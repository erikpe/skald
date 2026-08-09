# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

[Panic Runtime Trace Roadmap](PANIC_RUNTIME_TRACE_ROADMAP.md) is in progress;
the version-9 runtime, source-aware metadata, inline TLS frame maintenance,
source-call/failure location replacement, and generated-helper/runtime-failure
attribution are complete. Default-on request/CLI exposure and exact native
observation migration are complete; performance measurement and rollout
closeout are next. It
implements the archived frozen design through complete source location
attribution, default-on CLI/golden coverage, and measured closeout. It has no
dependency on another active roadmap; Linux AArch64 and recoverable exceptions
remain outside its scope.

## Planned

No implementation roadmap is currently only planned.

## Design proposals

No design proposal is currently under review. The frozen panic runtime-trace
design and its supporting investigation are preserved in the
[archive](../archive/README.md) and feed the planned implementation roadmap
above.

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
