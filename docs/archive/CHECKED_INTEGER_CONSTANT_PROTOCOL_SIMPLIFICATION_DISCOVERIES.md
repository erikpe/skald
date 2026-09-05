# Checked Integer Constant Protocol Simplification Discoveries

Status: resolved by CLR4 of the completed
[convergent local constant propagation roadmap](CONVERGENT_LOCAL_CONSTANT_PROPAGATION_ROADMAP.md),
following the frozen
[convergent local constant propagation design](CONVERGENT_LOCAL_CONSTANT_PROPAGATION_DESIGN_PROPOSAL.md)
and the completed checked-integer foundation's
[roadmap](CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md).

The [optimization candidate catalog](../roadmaps/OPTIMIZATION_CANDIDATE_CATALOG.md) owns
concise cross-domain placement, effort, value, prerequisite, and status
summaries; this record retains only implementation-specific evidence for
nested checked results that cross private scalar carriers.

## Nested successful protocols do not feed enclosing scalar carriers

Status: resolved; nested successful protocols now feed enclosing carriers.

**Former evidence:** Division/remainder coverage for
`((8 / 2) + (7 % 3)) / 2` formerly folded only the two independent inner
protocols, while `(1 << 2u) << 1u` formerly left the outer shift checked. The
old candidate query accepted only an exact literal assignment as a carrier's
unique store source.

**Resolution:** CLR4 makes the checked consumer combine immutable structural
topology with convergent solver facts and narrow carrier-plan evidence. It
plans dependent candidates together, revalidates the complete callable
snapshot and all conflicts before mutation, and commits once. Lowering also
reuses an existing checked-result carrier for sibling preservation, while the
carrier certificate accepts multiple exact dominated loads without accepting
generic storage. Both examples now fold completely in one checked occurrence.

**Accepted owner:** The frozen convergent design gives one seal-local dependency
graph and worklist solver ownership of constant provenance, while the checked-
protocol query retains structural topology and the existing passes retain
separate mutation authority. The existing read-only redundancy census provides
initial proof vocabulary but does not authorize mutation.

**Priority:** Promoted for architectural completeness. The version-one
[local-redundancy study](../archive/LOCAL_MIR_REDUNDANCY_MEASUREMENT_REPORT.md#candidate-comparison)
confirmed 25 safe carrier substitutions, but every one belongs to the focused
checked-protocol fixture. No standard-library, solver, control-flow,
whole-world, or benchmark workload supplied a proven final site, so the
candidate was not selected on measured performance benefit. The later design
decision is instead motivated by the expectation that every expression formed
entirely from supported constant operations should fold independent of its
nesting depth. The measurement result remains valid and is not being recast as
performance evidence.

**Accepted design boundary:** Prove constants through canonical private scalar-
spill store/load chains with explicit protocol ownership, access, write,
dominance, type, alias, and lifecycle conditions. Use those relations in a
monotonic callable-local solver which reasons through supported primitive and
successful checked operations to arbitrary graph depth before mutation. Let
the existing independently selectable primitive and checked passes materialize
only their own rewrite families. Do not recursively mutate nested diamonds,
rerun the whole pass to convergence, or broaden the solver into general
load/store propagation. The linked frozen design owns the full boundary; the
completed roadmap records delivery; CLR4 closed this finding.
