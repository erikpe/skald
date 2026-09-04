# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

The
[local final-MIR redundancy measurement roadmap](LOCAL_MIR_REDUNDANCY_MEASUREMENT_ROADMAP.md)
is in progress. Its
[frozen measurement contract](LOCAL_MIR_REDUNDANCY_MEASUREMENT_CONTRACT.md)
defines whole-world compilation identity, verified input/pre-reachability/final
snapshots, census accounting and overlaps, the version-one corpus, canonical
report schema, and evidence threshold. Composable verified-MIR inspection
through the driver is next.

## Pending discoveries

The living
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) inventories
implemented and possible later optimizations by HIR/lowering, final-MIR value,
CFG, storage/alias/ownership, whole-world execution, static-lifecycle,
target-LIR, and machine-artifact graph. Each entry records placement,
lifecycle status, effort, value, prerequisites, and pitfalls. Status
distinguishes implemented, in-progress, frozen proposed, draft-design,
follow-up, foundation-dependent, contract-dependent, and research work. It is
not an implementation roadmap.

The open
[local final-MIR redundancy measurement discoveries](LOCAL_MIR_REDUNDANCY_MEASUREMENT_DISCOVERIES.md)
records out-of-scope findings from the active measurement work. Ordinary
census results belong in its durable report, not in the discoveries record.

The open
[local final-MIR simplification discoveries](LOCAL_FINAL_MIR_SIMPLIFICATION_DISCOVERIES.md)
record implementation-specific proof-normalization, checked-operation,
floating-evaluation, memory-analysis, and measurement follow-ups found while
delivering the completed local simplification layer. Cataloged follow-ups do
not become roadmap work merely by being mentioned there.

The open
[checked integer constant protocol simplification discoveries](CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_DISCOVERIES.md)
records implementation-specific findings from the completed checked-protocol
pass without widening its reviewed successful-constant boundary. The
nested-carrier follow-up remains actionable.

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
backend lowering, and default activation after the canary. Its follow-ups are
resolved in the archived
[discoveries record](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DISCOVERIES.md).
The other four original unresolved constraints have no implementation roadmap.
The completed
[local final-MIR simplification roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md)
then adds exact primitive folding, guarded algebraic value forwarding,
proof-aware ordinary CFG cleanup, and the repeated default schedule. Its
[frozen design](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md)
and delivery record are preserved in the archive.

The completed interface-based operator-overloading, general-iteration, and
generic-interface roadmaps are preserved in the
[archive](../archive/README.md).

## Planned

No additional implementation roadmap is currently planned behind the active
local final-MIR redundancy measurement work.

## Design proposals

The completed checked integer constant protocol simplification delivery record
is preserved in the [archive](../archive/README.md). FMC-01 and FMC-02 are
implemented by the independently selectable
`checked-integer-constant-folding` pass; current behavior is authoritative in
the living [compiler phase](../compiler/PHASES_AND_IR.md#checked-integer-constant-protocol-simplification),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#final-mir-optimization-selection),
[reporting](../compiler/REPORTING.md#checked-integer-constant-protocol-simplification-observation),
and [testing](../development/TESTING.md#checked-integer-constant-protocol-simplification-coverage)
contracts.

The completed local final-MIR simplification design and delivery record are
preserved in the [archive](../archive/README.md). Their independently
selectable primitive constant folding, guarded algebraic forwarding, and
proof-aware CFG cleanup passes are authoritative in the living
[compiler phase](../compiler/PHASES_AND_IR.md#local-final-mir-simplification),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#local-final-mir-simplification-selection),
and
[reporting](../compiler/REPORTING.md#local-final-mir-simplification-observation)
contracts.

The completed reachability-gated static lifecycle design and delivery record
are preserved in the [archive](../archive/README.md). Their exact mandatory
entry-rooted active-field closure is authoritative in the living
[language](../language/STATIC_FIELDS.md#frozen-reachability-gated-activation-direction)
and
[compiler](../compiler/PHASES_AND_IR.md#frozen-reachability-gated-static-lifecycle-direction)
contracts.

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
