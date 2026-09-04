# Local Final-MIR Simplification Discoveries

Status: open; proof-provenance normalization is the sole remaining follow-up
from the completed
[local final-MIR simplification roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md).

The frozen
[design](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md) owns the
implemented scalar and conservative CFG behavior. The
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) owns
concise cross-domain placement, effort, value, and status. This record retains
only the implementation-specific proof/CFG evidence needed for a future
normalization design. That design is now the draft
[proof-provenance normalization proposal](PROOF_PROVENANCE_NORMALIZATION_DESIGN_PROPOSAL.md).

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

**Likely owner:** A proof-provenance classification and post-proof
normalization boundary shared by MIR rewriting and verification. The draft
[design proposal](PROOF_PROVENANCE_NORMALIZATION_DESIGN_PROPOSAL.md) inventories
the records, separates proof-rich and executable seals, and defines a
mandatory one-way normalizer. It is the
prerequisite for the catalog's proof-record deletion, empty-block forwarding,
block merging, jump threading, logical CFG simplification, and complete
unreachable-region deletion candidates.

**Priority:** Draft design under review. Checked
integer protocol folding now demonstrates atomic proof-aware CFG rewriting,
while conservative CFG cleanup continues to count and retain proof-protected
unreachable blocks. Implementation must wait for the proposal to be frozen
and promoted into a roadmap.

**Proposed bounded direction:** After complete proof-rich verification,
atomically lower path-condition reads to ordinary loads, reclassify their
activation storage, consume path-condition and logical-expression records,
and seal a distinct executable final-MIR product. Validate the boundary with
entry-unreachable block cleanup while deferring forwarding, merging,
threading, and checked-protocol normalization. Proof-named blocks remain
conservative roots until the proposal is implemented.
