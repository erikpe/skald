# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

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

The [optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md)
record the seven assessed compiler constraints on modular target-independent
and target-specific optimization, their interaction with permanent whole-world
and single-threaded program semantics, expected impact and effort, and a
recommended sequence. The completed
[static-lifecycle certificate roadmap](../archive/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
and the completed
[dense callable-local MIR identity rewriting roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
are preserved in the archive. The completed
[selectable final-MIR pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
adds the enabling layer around those foundations. The completed
[whole-world reachability roadmap](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md)
adds reusable whole-program roots and dependency analysis, independently
verified sparse definitions, target-independent semantic retention before
backend lowering, and default activation after the canary. Its follow-ups are
resolved in the archived
[discoveries record](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DISCOVERIES.md).
The proof/executable layering constraint is resolved by the completed
[proof-provenance normalization roadmap](../archive/PROOF_PROVENANCE_NORMALIZATION_ROADMAP.md),
which adds the mandatory two-seal boundary and final-stage unreachable
block/value cleanup. The other three original unresolved constraints have no
implementation roadmap.
The completed
[local final-MIR simplification roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md)
then adds exact primitive folding, guarded algebraic value forwarding,
proof-aware ordinary CFG cleanup, and the repeated default schedule. Its
[frozen design](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md)
and delivery record are preserved in the archive.
The completed
[post-proof CFG canonicalization roadmap](../archive/POST_PROOF_CFG_CANONICALIZATION_ROADMAP.md)
adds exact predecessor-edge facts, guarded final-CFG edits, independently
selectable empty-block forwarding and basic-block merging, deterministic
composition and observation, and source-to-native equivalence. Its
[frozen design](../archive/POST_PROOF_CFG_CANONICALIZATION_DESIGN_PROPOSAL.md)
and delivery record are preserved in the archive; implementation produced no
remaining roadmap-specific follow-up.
The completed
[convergent local constant propagation roadmap](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_ROADMAP.md)
adds one convergent callable-local solver, exhaustively certified checked-
protocol carriers, depth-independent primitive and checked consumers, and
constant-left logical selection through the proof-consuming transition. Its
[frozen design](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_DESIGN_PROPOSAL.md),
[resolved discoveries](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_DISCOVERIES.md),
and the resolved
[checked-protocol discovery](../archive/CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_DISCOVERIES.md)
are preserved in the archive.
The completed
[normalization-stable path-activation provenance roadmap](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_ROADMAP.md)
adds a final-only activation storage role, exact normalized initialization
authority, same-snapshot analysis classification, guarded rewrite ownership,
and target/observation parity. Its
[inventory](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_INVENTORY.md),
[resolved companion record](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_DISCOVERIES.md),
and the resolved original
[proof-normalization discovery](../archive/PROOF_PROVENANCE_NORMALIZATION_DISCOVERIES.md)
are preserved in the archive. FMM-13 remains explicitly unimplemented in the
living optimization candidate catalog.

The completed interface-based operator-overloading, general-iteration, and
generic-interface roadmaps are preserved in the
[archive](../archive/README.md).

## Planned

The in-progress
[indexed array construction roadmap](INDEXED_ARRAY_CONSTRUCTION_ROADMAP.md)
implements the frozen `T[](length; index => expression)` and shared-outer
forms through verified dynamic-prefix MIR, then adds the ordinary generic
`Vec<T>.to_array()` adopter. Frontend source retention, scoped resolution, and
typed repeated destination selection are complete; verified primitive dynamic-
prefix execution is next. The completed explicit element-list construction
pipeline is its implementation foundation.

## Design proposals

The completed
[indexed array construction design](../archive/INDEXED_ARRAY_CONSTRUCTION_DESIGN_PROPOSAL.md)
is frozen, promoted into the living language and compiler contracts, and
preserved in the archive. Its explicit `T[](length; index => expression)` and
shared-outer forms use direct destination initialization, per-element cleanup,
a verified dynamic initialized prefix, and no runtime ABI or `Vec` compiler
special case. The active
[implementation roadmap](INDEXED_ARRAY_CONSTRUCTION_ROADMAP.md) owns delivery;
verified primitive dynamic-prefix execution is next.

The completed
[normalization-stable path-activation provenance design](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_DESIGN_PROPOSAL.md)
and [delivery record](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_ROADMAP.md)
are preserved in the archive. Their dedicated final-only activation storage
kind, normalizer-exclusive construction, explicit proof/final phase legality,
and exact normalized definite-initialization authority are authoritative in
the living compiler documentation. Dead condition-carrier deletion remains a
separate optimization candidate.

The completed convergent local constant propagation
[design](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_DESIGN_PROPOSAL.md),
[delivery record](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_ROADMAP.md),
and [resolved discoveries](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_DISCOVERIES.md)
are preserved in the archive. The living compiler phase, driver, reporting,
testing, and optimization catalog documents are authoritative for current
behavior.

The completed post-proof CFG canonicalization
[design](../archive/POST_PROOF_CFG_CANONICALIZATION_DESIGN_PROPOSAL.md) and
[delivery record](../archive/POST_PROOF_CFG_CANONICALIZATION_ROADMAP.md) are
preserved in the archive. Their normalized CFG facts, permanent-attachment
barriers, narrow final-stage mutation authority, deterministic dense commit,
reverification, fresh reachability, and independently selectable forwarding
and merging passes are authoritative in the living
[compiler phase](../compiler/PHASES_AND_IR.md#proof-provenance-normalization-boundary),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#final-mir-optimization-selection),
[reporting](../compiler/REPORTING.md#post-proof-cfg-canonicalization-observation),
and [testing](../development/TESTING.md#proof-provenance-normalization-coverage)
contracts.

The completed proof-provenance normalization
[design](../archive/PROOF_PROVENANCE_NORMALIZATION_DESIGN_PROPOSAL.md) and
[delivery record](../archive/PROOF_PROVENANCE_NORMALIZATION_ROADMAP.md) are
preserved in the archive. Their mandatory two-seal transition, stage-aware
pipeline, normalized backend boundary, and conservative post-proof CFG cleanup
are authoritative in the living compiler documentation.

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
