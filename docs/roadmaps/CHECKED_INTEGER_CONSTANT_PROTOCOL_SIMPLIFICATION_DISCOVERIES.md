# Checked Integer Constant Protocol Simplification Discoveries

Status: open follow-up record from the completed
[checked integer constant protocol simplification roadmap](../archive/CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md).

Use this file for concrete maintainability findings or optimization
opportunities discovered while implementing the roadmap that do not belong in
its reviewed FMC-01/FMC-02 scope. Each finding should record the problem,
implementation evidence, likely owner, priority, and a bounded future
direction.

Do not duplicate the complete
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md). The
catalog owns concise cross-domain placement, effort, value, prerequisite, and
status summaries. This record owns implementation-specific evidence needed to
make a later task reviewable.

Expected but not pre-approved topics include direct folding of statically
failing protocols, eliminating a successful check around a dynamic operation,
nested checked-constant propagation, redundant private scalar-spill cleanup,
proof-provenance normalization, and broader checked scalar families. Recording
a topic here does not reopen the completed roadmap.

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

**Likely owner:** A future verified scalar-spill constant-provenance or narrow
storage-propagation analysis shared by final-MIR simplifications, rather than
the checked-protocol topology query itself.

**Priority:** Low. The version-one
[local-redundancy study](LOCAL_MIR_REDUNDANCY_MEASUREMENT_REPORT.md#candidate-comparison)
confirmed 25 safe carrier substitutions, but every one belongs to the focused
checked-protocol fixture. No standard-library, solver, control-flow,
whole-world, or benchmark workload supplied a proven final site, so the
candidate was measured but not selected for an implementation project.

**Bounded direction:** Prove constants through canonical private scalar-spill
store/load chains with explicit write, dominance, type, alias, and lifecycle
conditions. Keep that fact local to one verified seal and let the existing
checked-protocol query consume it; do not recursively rewrite nested diamonds
or broaden CIR3 into general load/store propagation.
