# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

[Produced-object field reads](PRODUCED_OBJECT_FIELD_READS_ROADMAP.md) is in
progress; PFR4 is next. Every implemented readable field category now
type-checks and lowers through one ordinary produced temporary, including
optional, array, shared-owner, optional-owner, and optional-box copies,
transfers, aliases, anchors, guards, assignment sources, and results. Canonical
projections retain exact origin and every independent result is secured before
root cleanup. Comprehensive lifetime and control-flow verifier hardening is
the next boundary. The roadmap depends on the completed produced receiver,
object-cast, ownership, optional, array, generic-class, and structural
indexing foundations.

## Planned

No additional implementation roadmap is currently planned.

## Design proposals

No design proposal is currently under review. The confirmed structural
indexing and slicing decisions are preserved in the
[archive](../archive/STRUCTURAL_INDEXING_AND_SLICING_DESIGN_PROPOSAL.md).

Frozen design proposals and their completed implementation roadmaps are
preserved in the [archive](../archive/README.md).

## Pending discoveries

[Generic array copy lifecycle](GENERIC_ARRAY_COPY_LIFECYCLE_DISCOVERY.md)
records an internal MIR failure when an explicit generic copy body initializes
a type-parameter field specialized to an array. Synthesized lifecycle remains
the current workaround; the eventual fix belongs to generic-class lifecycle
lowering rather than structural indexing.

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
