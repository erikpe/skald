# Local Final-MIR Simplification Discoveries

Status: open follow-up record for the planned
[local final-MIR simplification roadmap](LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md).
No implementation discoveries have been recorded yet.

Use this file for concrete maintainability or follow-up findings discovered
while implementing the frozen
[design](LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md) that do not belong
in its reviewed scope. Each finding should record the problem, implementation
evidence, likely owner, priority, and a bounded future direction.

Do not duplicate the complete
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md). That
catalog owns concise cross-domain placement, effort, value, and status. This
record should contain only implementation-specific evidence and follow-up
detail needed to make a later task reviewable. When a discovery becomes a
confirmed design or roadmap, update its catalog status and link to the new
authority.

Expected but not pre-approved topics include checked integer division,
remainder and shift folding; always-successful checked-protocol elimination;
floating evaluation; proof-provenance normalization; empty-block forwarding;
storage propagation; alias/effect analysis; and measurement-driven schedule
adjustments. Recording a topic here does not add it retroactively to the
active roadmap.
