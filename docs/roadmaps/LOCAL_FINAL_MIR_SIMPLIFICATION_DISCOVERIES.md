# Local Final-MIR Simplification Discoveries

Status: open follow-up record from the completed
[local final-MIR simplification roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md).

Use this file for concrete maintainability or follow-up findings discovered
while implementing the frozen
[design](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md) that did
not belong in its reviewed scope. Each finding should record the problem,
implementation evidence, likely owner, priority, and a bounded future
direction.

Do not duplicate the complete
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md). That
catalog owns concise cross-domain placement, effort, value, and status. This
record should contain only implementation-specific evidence and follow-up
detail needed to make a later task reviewable. When a discovery becomes a
confirmed design or roadmap, update its catalog status and link to the new
authority.

Expected but not pre-approved topics include always-successful dynamic
checked-protocol elimination;
floating evaluation; proof-provenance normalization; empty-block forwarding;
storage propagation; alias/effect analysis; and measurement-driven schedule
adjustments. Recording a topic here does not add it retroactively to the
completed roadmap.

## Proof-coupled logical CFG remains intentionally opaque

**Evidence:** The focused optimization golden exercises an ordinary branch
made constant by local folding next to a logical expression whose proof
metadata still names its CFG region. Conservative CFG cleanup can fold and
remove the former, but must retain the latter even when ordinary entry
reachability no longer reaches it. The verifier and lifecycle certificate
depend on those proof-owned block identities.

**Impact:** More aggressive branch deletion, empty-block forwarding, or jump
threading cannot be added as local special cases without risking stale logical
records, path conditions, guards, or lifecycle evidence.

**Likely owner:** A future proof-provenance normalization design shared by MIR
rewriting and verification; catalog candidates FMC-07, FMC-11, and FMC-12.

**Priority:** Medium after checked-protocol folding has demonstrated demand.

**Bounded direction:** Define an atomic proof/CFG rewrite transaction that can
replace or remove every affected proof identity before extending CFG cleanup.
Until then, proof-named blocks remain conservative roots.

## Checked and memory-dependent families need separate semantics

**Evidence:** Focused coverage exercises wrapping integer extrema and `u8` canonicalization
in debug and release builds, but deliberately leaves integer division,
remainder, shifts, checked conversions, floating evaluation, loads, and stores
unchanged. Checked operations are coupled to failure diamonds; memory reads
and writes require alias and effect reasoning that block-local scalar facts do
not provide.

**Impact:** These omitted families can have substantial value, but expanding
the current evaluator or algebraic table would conflate total scalar
evaluation with control effects or mutable state.

**Likely owner:** The completed
[checked integer constant protocol simplification roadmap](../archive/CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md)
delivered FMC-01 and FMC-02. Later checked integer and conversion extensions own
FMC-03 through FMC-06; deterministic floating evaluation owns FMV-05 through
FMV-07; memory and storage reasoning owns FMM-01 through FMM-06 in the
optimization candidate catalog.

**Priority:** FMC-01 and FMC-02 are implemented. Priority remains medium or
lower for floating and memory families pending workload measurements and their
prerequisite designs.

**Bounded direction:** The implemented pass treats a statically successful
checked protocol as the rewrite unit, not only its success rvalue. Design
deterministic IEEE evaluation before folding floating operations. Introduce
explicit alias/effect facts before any general load/store propagation.

## Initial measurements separate local wins from whole-world pruning

**Evidence:** The compact local-simplification pipeline fixture starts with 3 executable
definitions, 5 blocks, 14 instructions, and 14 transient values. The complete
default schedule leaves 2 definitions, 3 blocks, 3 instructions, and 3 values.
Each local pass reports a non-zero reason counter, and reachability attributes
the one removed definition.

The standard-library-backed optimization golden starts with 147 executable
definitions, 2,139 blocks, and 10,068 instructions. Across the repeated
schedule, primitive folding reports 24 folded assignments, algebraic
simplification forwards 3 uses, CFG cleanup removes 7 blocks and 3 transient
values, and whole-world reachability removes 118 executable definitions. The
final program has 29 definitions, 99 blocks, and 561 instructions.

**Impact:** Whole-world retention dominates this particular program's gross
size reduction, while the local passes demonstrably expose smaller scalar and
CFG wins. These structural observations are useful for choosing follow-ups but
are neither runtime measurements nor representative workload rankings.

**Likely owner:** Pipeline occurrence measurements and future benchmark
corpora; the candidate catalog remains the concise prioritization register.

**Priority:** Ongoing.

**Bounded direction:** Keep deterministic structural counters on focused and
standard-library-backed fixtures. Add timing or workload rankings only through
a separately reviewed benchmark methodology; never gate correctness tests on
wall-clock results.
