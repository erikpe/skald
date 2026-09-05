# Proof-Provenance Normalization Discoveries

Status: open; the remaining storage-provenance follow-up now has a frozen
[design](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_DESIGN_PROPOSAL.md)
and a planned
[implementation roadmap](NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_ROADMAP.md).

The archived
[design](../archive/PROOF_PROVENANCE_NORMALIZATION_DESIGN_PROPOSAL.md) owns the reviewed
classification, two-seal boundary, mandatory normalization, stage-aware
pipeline, backend migration, and conservative post-proof cleanup. The
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) owns
broader optimization placement and status.

This file retains only evidence that still requires later work. The catalog's
[dead normalized condition-carrier storage cleanup](OPTIMIZATION_CANDIDATE_CATALOG.md#final-mir-storage-alias-effect-and-ownership-graph)
entry owns its cross-domain status and placement.

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
with normalized verification; medium priority now that the proof boundary and
its current consumers are stable, rising to high before a final-stage storage
or spill transformation.

**Proposed resolution:** The
[frozen normalization-stable path-activation provenance design](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_DESIGN_PROPOSAL.md),
implemented through the planned
[roadmap](NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_ROADMAP.md),
adds a dedicated final-only storage kind produced solely by the mandatory
normalizer, restores ordinary `ScalarSpill` definite-initialization checking in
normalized MIR, and retains consumed-proof authority only for the exact marked
activation class. It does not retain erased path identities or include the
later dead-carrier optimization.
