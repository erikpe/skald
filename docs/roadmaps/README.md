# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

[Shared optional boxes](SHARED_OPTIONAL_BOXES_ROADMAP.md) is in progress; BX0
through BX8 are complete and BX9 is next. It implements the frozen non-null `shared P?` box
and derived `shared? P?` optional owner through canonical targets, verified
immutable wrapper access, polymorphic class/interface/`Obj` views, exact
metadata and finalization, stored positions, and arrays while retaining
runtime ABI version 9. It depends on the completed compositional optional,
shared ownership, explicit shared dereference, object cast, array, and
static-field profiles.

## Planned

No implementation roadmap is currently planned but not started.

## Design proposals

No design proposal is currently under review.

Frozen design proposals and their completed implementation roadmaps are
preserved in the [archive](../archive/README.md).

## Pending discoveries

The [shared optional boxes discoveries](SHARED_OPTIONAL_BOXES_DISCOVERIES.md)
record contains a control-flow value-stabilization issue found while composing
the BX8 native matrix. It is deliberately separate from the frozen box and
array ownership work.

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
