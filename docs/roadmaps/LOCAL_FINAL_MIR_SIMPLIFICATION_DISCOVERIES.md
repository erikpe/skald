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
normalization design.

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
normalization design shared by MIR rewriting and verification. It is the
prerequisite for the catalog's proof-record deletion, empty-block forwarding,
block merging, jump threading, logical CFG simplification, and complete
unreachable-region deletion candidates.

**Priority:** High for the next architecture design investigation. Checked
integer protocol folding now demonstrates atomic proof-aware CFG rewriting,
while conservative CFG cleanup continues to count and retain proof-protected
unreachable blocks. Implementation must wait for the normalization contract.

**Bounded direction:** Inventory every proof-bearing MIR record and its final
semantic consumer. Classify each as permanently semantic, consumable, or
recomputable; define the verified product after consumption; and provide one
atomic proof/CFG rewrite transaction that removes or replaces all affected
identities. Only then extend CFG cleanup. Proof-named blocks remain
conservative roots until that boundary exists.
