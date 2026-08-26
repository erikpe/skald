# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

No implementation roadmap is currently in progress.

The completed general iteration and generic interfaces roadmaps are preserved
in the [archive](../archive/README.md).

## Planned

No additional implementation roadmap is currently waiting to start.

## Design proposals

The [operator-overloading design proposal](OPERATOR_OVERLOADING_DESIGN_PROPOSAL.md)
is an active draft. It explores canonical `std::ops` generic interfaces,
compiler-provided primitive implementations, exact class and generic-bound
selection, and read-only primitive temporary materialization. The next design
work is to settle canonical naming and language-item acquisition. Operator
selection requires one unique applicable protocol and performs no specificity
ranking; multiple applicable generic bounds are definition-site ambiguity.
Typed `OpEq<Rhs>` remains separate from dynamic `Equatable` and
derives `!=` through one negation; prefix `!` itself is not overloadable. Four
direct boolean protocols own ordering without complement or operand reversal,
and compiler-provided `f64` implementations preserve existing IEEE-754
unordered comparison rather than `BoxF64` bit equality. Generic interfaces
and general iteration are implemented dependencies; no implementation roadmap
should be created until the proposal is frozen and promoted.

The completed general iteration design and delivery record are preserved in
the [archive](../archive/README.md).

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
