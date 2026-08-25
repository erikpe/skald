# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

The [general iteration roadmap](GENERAL_ITERATION_ROADMAP.md) is in progress.
Its canonical dependency-free `std::iter::Iterable<Item, State>` declaration,
typed compiler-dependency evidence, structural language-item validation,
exact resolved identities, deterministic dump, and focused tests are complete.
`IT1` is next: reserve and parse the `for-in` source form and attach its spans
to the canonical module dependency without creating a source binding.

The completed generic interfaces roadmap is preserved in the
[archive](../archive/GENERIC_INTERFACES_ROADMAP.md).

## Planned

No additional implementation roadmap is currently waiting to start.

## Design proposals

The frozen [general iteration design proposal](GENERAL_ITERATION_DESIGN_PROPOSAL.md)
defines nominal `Iterable<Item, State>` selection, structured
`for (item in iterable)` semantics, loop-duration receiver and state
lifetimes, optional termination, loop exits, phase boundaries, and ordinary
`Vec<T>` adoption. Its complete decision register was confirmed on 2026-08-25,
promoted into focused living language and compiler contracts, and translated
into the active implementation roadmap above. Operator overloading, numeric
ranges, range syntax, and intrinsic array conformance remain explicit future
consumers rather than part of this proposal.

The confirmed generic interfaces decisions and completed delivery history are
preserved in the
[archive](../archive/GENERIC_INTERFACES_DESIGN_PROPOSAL.md) and
[completed roadmap](../archive/GENERIC_INTERFACES_ROADMAP.md), and promoted
into focused implemented language and compiler contracts.

The completed private cell fields design and implementation roadmap are
preserved in the [archive](../archive/PRIVATE_CELL_FIELDS_DESIGN_PROPOSAL.md).

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
