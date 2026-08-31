# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

The
[reachability-gated static lifecycle roadmap](REACHABILITY_GATED_STATIC_LIFECYCLE_ROADMAP.md)
changes runtime lifecycle from declaration-wide eager activation to one exact
entry-rooted active-field closure while keeping declarations and preliminary
initializer checking whole-world. RSR0 established the private activation
vocabulary and pinned current eager behavior; RSR1 centralized direct static-
place extraction in the shared dependency inventory; RSR2 is next. It depends
on the completed static-lifecycle certificate, selectable final-MIR pipeline,
dense MIR rewriting, and target-independent reachability foundations.

## Pending discoveries

The [optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md)
record the seven current compiler constraints on modular target-independent and
target-specific optimization, their interaction with permanent whole-world and
single-threaded program semantics, expected impact and effort, and a recommended
starting sequence. Its first two recommended changes are now implemented; the
[completed static-lifecycle certificate roadmap](../archive/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
and the completed
[dense callable-local MIR identity rewriting roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
are preserved in the archive. The completed
[selectable final-MIR pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
adds the enabling layer around those foundations and activates one conservative
default pass. The completed
[whole-world reachability roadmap](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md)
adds reusable whole-program roots and dependency analysis, independently
verified sparse definitions, target-independent semantic retention before
backend lowering, and default activation after the canary. Its remaining
follow-ups stay in the active
[discoveries record](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DISCOVERIES.md).
The other four original unresolved constraints have no implementation roadmap.

The
[reachability-gated static lifecycle discoveries](REACHABILITY_GATED_STATIC_LIFECYCLE_DISCOVERIES.md)
will retain out-of-scope findings from the active semantic migration. It is
currently empty and does not expand the frozen contract or roadmap.

The completed interface-based operator-overloading, general-iteration, and
generic-interface roadmaps are preserved in the
[archive](../archive/README.md).

## Planned

No additional implementation roadmap is currently planned.

## Design proposals

The frozen
[reachability-gated static lifecycle design](REACHABILITY_GATED_STATIC_LIFECYCLE_DESIGN_PROPOSAL.md)
changes class-owned statics from import-wide eager activation to one exact,
mandatory, field-grained activation closure rooted at the selected entry.
Active fields will still initialize eagerly before entry and shut down in
exact reverse order; declarations and preliminary initializer checking would
remain whole-world. The design adds no eager/module-initialization syntax. Its
decisions are promoted into the living
[language](../language/STATIC_FIELDS.md#frozen-reachability-gated-activation-direction)
and
[compiler](../compiler/PHASES_AND_IR.md#frozen-reachability-gated-static-lifecycle-direction)
contracts, and implementation is scheduled by the active roadmap above.

The completed selectable final-MIR optimization pipeline design and delivery
record are preserved in the [archive](../archive/README.md). Their typed static
registry, deterministic profiles and schedules, request and CLI selection,
verified pass ownership, structured measurements, verified checkpoints, and
default dead-pure-definition elimination canary are promoted into the living
[compiler phase](../compiler/PHASES_AND_IR.md#selectable-final-mir-optimization-pipeline),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#final-mir-optimization-selection),
and [reporting](../compiler/REPORTING.md#final-mir-pass-reporting)
contracts.

The completed target-independent whole-world reachability design and delivery
record are preserved in the [archive](../archive/README.md). Their implemented
root, dependency, closure, verified sparse-definition, retention, backend, and
selection contracts are authoritative in the living
[compiler phase](../compiler/PHASES_AND_IR.md#target-independent-whole-world-reachability),
[backend](../compiler/BACKEND.md#target-independent-reachability-boundary),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#whole-world-reachability-selection),
and [reporting](../compiler/REPORTING.md#whole-world-reachability-observation)
documentation.

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
