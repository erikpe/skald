# Checked Integer Constant Protocol Simplification Discoveries

Status: open; the one measured follow-up from the completed
[checked integer constant protocol simplification roadmap](../archive/CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md)
is accepted by the frozen
[convergent local constant propagation design](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_DESIGN_PROPOSAL.md)
and tracked by its planned
[implementation roadmap](CONVERGENT_LOCAL_CONSTANT_PROPAGATION_ROADMAP.md).

The [optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) owns
concise cross-domain placement, effort, value, prerequisite, and status
summaries; this record retains only implementation-specific evidence for
nested checked results that cross private scalar carriers.

## Nested successful protocols do not feed enclosing scalar carriers

**Evidence:** Division/remainder coverage for `((8 / 2) + (7 % 3)) / 2` finds
and folds the two independent inner protocols in one callable transaction;
shift coverage observes the same boundary for `(1 << 2u) << 1u`. The enclosing
operation remains checked on the next observation: each inner constant is
stored into its result carrier, reloaded at its join, and then stored into an
outer operand carrier, while the deliberately narrow candidate query accepts
only an exact constant assignment as the unique carrier-store source.

**Impact:** Correctness and idempotence are unaffected, but nested constant
checked expressions can leave optimization opportunities behind even after
their inner operations have folded. Repeating the same checked-protocol pass
cannot expose the outer operation without an additional propagation rule.

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

**Accepted resolution:** Prove constants through canonical private scalar-
spill store/load chains with explicit protocol ownership, access, write,
dominance, type, alias, and lifecycle conditions. Use those relations in a
monotonic callable-local solver which reasons through supported primitive and
successful checked operations to arbitrary graph depth before mutation. Let
the existing independently selectable primitive and checked passes materialize
only their own rewrite families. Do not recursively mutate nested diamonds,
rerun the whole pass to convergence, or broaden the solver into general
load/store propagation. The linked frozen design owns the full boundary; the
active roadmap owns implementation and closure of this finding.
