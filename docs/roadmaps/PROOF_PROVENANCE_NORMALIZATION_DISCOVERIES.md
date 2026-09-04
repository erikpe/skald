# Proof-Provenance Normalization Discoveries

Status: open for implementation discoveries from the active
[proof-provenance normalization roadmap](PROOF_PROVENANCE_NORMALIZATION_ROADMAP.md).

The frozen
[design](PROOF_PROVENANCE_NORMALIZATION_DESIGN_PROPOSAL.md) owns the reviewed
classification, two-seal boundary, mandatory normalization, stage-aware
pipeline, backend migration, and conservative post-proof canary. The
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) owns
broader optimization placement and status.

Record only evidence discovered while implementing the roadmap which should
not expand its active scope. Each finding should state the problem, concrete
evidence, impact, likely owner, priority, and a bounded later direction.
Resolve small directly supporting maintainability improvements in the task
that uncovers them. Do not use this file to add empty-block forwarding, block
merging, jump threading, logical CFG simplification, checked-protocol
normalization, storage deletion, alias/effect analysis, SSA, inlining, or
target optimization to the roadmap.

## Reclassified path activations lose their scalar-spill origin

**Problem:** The normalized representation deliberately reclassifies
`PathCondition` activation storage as the already existing `ScalarSpill`
kind. After proof records are consumed, ordinary scalar-initialization
dataflow can no longer distinguish those former path activations from spills
created by other compiler protocols.

**Concrete evidence:** Enabling the normalized lifecycle verifier for the new
final seal made valid short-circuit and conditional-cleanup programs fail
ordinary definite-initialization checks. Their activation loads are valid only
under the path proof which the mandatory transaction has just consumed. The
normalizer preserves the exact stores and loads, but the erased path records
were the evidence needed to re-prove initialization at those sites.

**Current bounded resolution:** Complete proof-rich verification still checks
every `ScalarSpill` before normalization. Final-seal construction is possible
only by consuming that seal through the complete transaction, and normalized
verification continues checking every source-visible primitive storage kind.
It excludes compiler-owned `ScalarSpill` storage from the general
definite-initialization analysis and relies on the consumed-proof authority
plus surviving checked-protocol validators for those internal carriers.
Focused tests cover both accepted normalized path carriers and rejected
uninitialized source locals.

**Impact:** This is sound for the frozen one-way transition and the planned
block/value-only canary, which neither creates nor moves storage accesses. It
would be too implicit if later final-stage passes begin synthesizing, moving,
or combining scalar-spill loads because the generic storage kind does not say
which protocol owns their initialization.

**Likely owner and priority:** MIR storage/protocol representation together
with normalized verification; low priority during this roadmap, rising to
high before a final-stage storage or spill transformation.

**Bounded later direction:** If such a pass is proposed, give compiler-owned
scalar carriers explicit surviving protocol ownership or another
normalization-stable provenance classification, then make normalized definite
initialization dispatch through that owner. Do not retain erased path records
or attach stale pre-normalization identities merely to recover this
distinction.
