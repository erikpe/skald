# Selectable Final-MIR Optimization Pipeline Discoveries

Status: active companion to the in-progress
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

### Mutation-oriented identity traversal makes read-only analyses snapshot MIR

- **Problem:** The authoritative exhaustive identity traversal rewrites
  identities in place, so a genuinely read-only consumer must currently map a
  private clone rather than borrow the active callable directly.
- **Evidence:** `mir::rewrite::map` accepts mutable MIR throughout, and the
  value-use census consequently clones `MirCallableEdit` before invoking
  `map_live_references`. A fixed-point optimization repeats that snapshot once
  per analysis wave.
- **Why deferred:** Introducing a shared immutable visitor/transform kernel
  would touch the complete exhaustive traversal and every mapper, importer,
  committer, substitution, and validation consumer. That is substantially
  broader and riskier than publishing the narrow census needed by the frozen
  pipeline.
- **Likely owner:** The callable-local identity traversal under
  `mir::rewrite`.
- **Priority:** Medium after the canary has supplied representative compile-time
  measurements; correctness and API isolation are unaffected.
- **Bounded next step:** Measure snapshot cost on broad MIR fixtures, then
  design a reviewable immutable-observer layer that shares the existing
  exhaustive destructuring and compile-time coverage without creating a
  second identity inventory.

## Resolution and closure

When a finding is implemented, replace it with a short resolution note and a
link to the authoritative living contract or archived delivery record. At
roadmap completion, move this file to `docs/archive/` only if actionable
follow-ups remain. If it is still empty, remove it and note that the final
review found no deferred pipeline findings in the archived roadmap.
