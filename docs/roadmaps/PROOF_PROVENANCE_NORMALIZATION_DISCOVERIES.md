# Proof-Provenance Normalization Discoveries

Status: open; the remaining storage-provenance follow-up has a frozen
[design](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_DESIGN_PROPOSAL.md)
and an active
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

**Problem:** The original normalized representation reclassified
`PathCondition` activation storage as the already existing `ScalarSpill`
kind. After proof records were consumed, ordinary scalar-initialization
dataflow could not distinguish those former path activations from spills
created by other compiler protocols.

**Concrete evidence:** Enabling the normalized lifecycle verifier for the new
final seal made valid short-circuit and conditional-cleanup programs fail
ordinary definite-initialization checks. Their activation loads are valid only
under the path proof which the mandatory transaction has just consumed. The
normalizer preserves the exact stores and loads, but the erased path records
were the evidence needed to re-prove initialization at those sites.

**Implemented core resolution:** Mandatory normalization now reclassifies the
carrier as the dedicated final-only `NormalizedPathActivation` kind. Ordinary
`ScalarSpill` declarations undergo definite-initialization analysis in both
verifier stages. Only a structurally valid normalized activation receives
consumed path-initialization trust, and only the private proof-consuming
pipeline can issue the authority needed to seal a final product. Focused tests
cover initialized and uninitialized ordinary spills, marked activations,
wrong-stage and malformed declarations, leaked proof, and valid normalized
programs. The discovery remains open until the active roadmap completes its
transformation, backend, and source-profile audits.

**Impact:** This is sound for the frozen one-way transition and the planned
block/value-only canary, which neither creates nor moves storage accesses. It
would be too implicit if later final-stage passes begin synthesizing, moving,
or combining scalar-spill loads because the generic storage kind does not say
which protocol owns their initialization.

**Likely owner and priority:** MIR storage/protocol representation together
with normalized verification; medium priority now that the proof boundary and
its current consumers are stable, rising to high before a final-stage storage
or spill transformation.

**Accepted resolution:** The
[frozen normalization-stable path-activation provenance design](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_DESIGN_PROPOSAL.md),
implemented through the active
[roadmap](NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_ROADMAP.md),
adds a dedicated final-only storage kind produced solely by the mandatory
normalizer, restores ordinary `ScalarSpill` definite-initialization checking in
normalized MIR, and retains consumed-proof authority only for the exact marked
activation class. It does not retain erased path identities or include the
later dead-carrier optimization.
