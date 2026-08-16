# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

No implementation roadmap is currently in progress.

## Planned

The [private cell fields roadmap](PRIVATE_CELL_FIELDS_ROADMAP.md) is planned to
implement narrow verified whole-field interior mutation for ordinary private
instance fields. CFI0, contextual declaration metadata, is next. It depends
on the implemented privacy, receiver-access, field-assignment, lifecycle,
alias-anchor, optional-guard, generic-specialization, and verified-MIR
contracts; no active roadmap blocks it. The separate `Str` cached-hash
migration depends on its completion and remains outside this roadmap.

## Design proposals

The [private cell fields design proposal](PRIVATE_CELL_FIELDS_DESIGN_PROPOSAL.md)
is frozen and promoted into the planned language and compiler contracts. Its
implementation is owned by the planned roadmap above; the separate `Str`
cached-hash language-item migration has not started.

The confirmed structural indexing and slicing decisions are preserved in the
[archive](../archive/STRUCTURAL_INDEXING_AND_SLICING_DESIGN_PROPOSAL.md).

The confirmed capture-free function-value decisions and completed
implementation roadmap are preserved in the
[archive](../archive/FUNCTION_VALUES_DESIGN_PROPOSAL.md) and promoted into the
focused living language and compiler contracts.

Frozen design proposals and their completed implementation roadmaps are
preserved in the [archive](../archive/README.md).

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
