# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

The
[selectable final-MIR optimization pipeline roadmap](SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
is in progress. Its typed registry, empty `none`/`default` profiles,
deterministic schedule occurrence model, exclusions, and crate-private exact
schedule resolver are implemented. Typed request and CLI selection is next.

## Pending discoveries

The [optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md)
record the seven current compiler constraints on modular target-independent and
target-specific optimization, their interaction with permanent whole-world and
single-threaded program semantics, expected impact and effort, and a recommended
starting sequence. Its first two recommended changes are now implemented; the
[completed static-lifecycle certificate roadmap](../archive/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
and the completed
[dense callable-local MIR identity rewriting roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
are preserved in the archive. The selectable final-MIR pipeline is now the
planned enabling layer around those foundations. The remaining five original
constraints stay pending and have no implementation roadmap.

The
[selectable pipeline discoveries](SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_DISCOVERIES.md)
are the active companion for maintainability or architecture findings exposed
by that roadmap. No deferred finding has been recorded yet.

The completed interface-based operator-overloading, general-iteration, and
generic-interface roadmaps are preserved in the
[archive](../archive/README.md).

## Planned

No other implementation roadmap is currently planned.

## Design proposals

The
[selectable final-MIR optimization pipeline proposal](SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_DESIGN_PROPOSAL.md)
defines a typed static pass registry, deterministic profiles and schedules,
verified pass ownership, request and CLI selection, structured measurements,
verified dump checkpoints, and a narrow dead-pure-definition elimination
canary. Its twelve decisions were frozen together on 2026-08-30 and promoted
into the living
[compiler phase](../compiler/PHASES_AND_IR.md#frozen-selectable-final-mir-optimization-pipeline-direction),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#frozen-final-mir-optimization-selection),
and [reporting](../compiler/REPORTING.md#frozen-final-mir-pass-reporting)
contracts. The active roadmap owns implementation; no production optimization
is implemented yet. The registry and schedule-policy foundation is current;
the remaining pipeline is in progress.

The completed dense callable-local MIR identity rewriting design and delivery
record are preserved in the [archive](../archive/README.md); the implemented
boundary is specified by the
[compiler phase contract](../compiler/PHASES_AND_IR.md#dense-callable-local-mir-identity-rewriting).

The static-lifecycle certificate decisions are promoted into the
[compiler phase contract](../compiler/PHASES_AND_IR.md#frozen-static-lifecycle-certificate-direction),
their
[frozen decision record](../archive/STATIC_LIFECYCLE_CERTIFICATE_DESIGN_PROPOSAL.md)
is preserved in the archive, together with the completed
[implementation roadmap](../archive/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md).

The structured-reporting decisions are promoted into the
[compiler reporting contract](../compiler/REPORTING.md),
their [frozen decision record](../archive/STRUCTURED_REPORTING_DESIGN_PROPOSAL.md)
and completed
[implementation roadmap](../archive/STRUCTURED_REPORTING_ROADMAP.md) are
preserved in the archive.

The frozen generic-range contract is promoted into the
[language](../language/RANGES.md) and [compiler](../compiler/RANGES.md)
documentation, and its
[decision record](../archive/GENERIC_RANGES_DESIGN_PROPOSAL.md) is preserved in
the archive.

The frozen interface-based operator-overloading contract is promoted into the
[language](../language/OPERATOR_OVERLOADING.md) and
[compiler](../compiler/OPERATOR_OVERLOADING.md) documentation, and its
decision record is preserved in the [archive](../archive/README.md).

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
