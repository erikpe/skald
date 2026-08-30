# Selectable Final-MIR Optimization Pipeline Discoveries

Status: active companion to the planned
[selectable final-MIR optimization pipeline roadmap](SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md).

This record holds maintainability, architecture, and follow-up findings found
while implementing the frozen
[selectable final-MIR optimization pipeline design](SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_DESIGN_PROPOSAL.md)
when they are valuable but too large or insufficiently coupled to the active
roadmap task. Small cohesive improvements should be implemented directly in
the task that exposes them.

The broader
[optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md)
remain the owner for already-known large optimizer directions such as
whole-program reachability, proof-provenance normalization, shared alias and
effect analysis, SSA or a separate optimization IR, and a virtual-register
target layer. Do not duplicate those topics here unless implementation reveals
new concrete evidence or changes their bounded next step.

## Recording rule

Each retained finding must include:

- **Problem:** the concrete maintainability or architecture issue;
- **Evidence:** files, types, tests, failure modes, or repeated work that make
  the issue observable;
- **Why deferred:** why fixing it would expand or destabilize the active task;
- **Likely owner:** the compiler subsystem or contract that should own it;
- **Priority:** high, medium, or low relative to other post-roadmap work; and
- **Bounded next step:** a reviewable investigation, design, or implementation
  action rather than an open-ended wish.

Findings should not use roadmap task codes or silently become prerequisites.
If a finding blocks the frozen pipeline contract, it belongs in the active
roadmap instead of this file.

## Active findings

No deferred findings have been recorded.

## Resolution and closure

When a finding is implemented, replace it with a short resolution note and a
link to the authoritative living contract or archived delivery record. At
roadmap completion, move this file to `docs/archive/` only if actionable
follow-ups remain. If it is still empty, remove it and note that the final
review found no deferred pipeline findings in the archived roadmap.
