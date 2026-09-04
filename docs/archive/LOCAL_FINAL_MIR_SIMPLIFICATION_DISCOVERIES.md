# Local Final-MIR Simplification Discoveries

Status: resolved by the completed proof-provenance normalization boundary and
ready for archival. This finding came from the completed
[local final-MIR simplification roadmap](LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md).

The frozen
[design](LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md) owns the
implemented scalar and conservative CFG behavior. The
[optimization candidate catalog](../roadmaps/OPTIMIZATION_CANDIDATE_CATALOG.md) owns
concise cross-domain placement, effort, value, and status. This record
preserves the implementation-specific proof/CFG evidence that led to the
archived
[proof-provenance normalization design](PROOF_PROVENANCE_NORMALIZATION_DESIGN_PROPOSAL.md).

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

**Resolution:** The completed
[proof-provenance normalization roadmap](PROOF_PROVENANCE_NORMALIZATION_ROADMAP.md)
implemented exhaustive proof-site classification, distinct proof-rich and
normalized seals, atomic proof consumption, stage-aware execution, and
post-proof unreachable block/value elimination. Proof-named blocks remain
roots only in the proof-rich region; normalized final MIR retains permanent
semantic roots instead.

Broader forwarding, merging, threading, and checked-protocol normalization
remain separate entries in the optimization candidate catalog rather than
unresolved work in this record.
