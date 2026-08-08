# Array Element-List Construction Discoveries

Status: one actionable follow-up remains after the completed array
element-list construction roadmap.

## Array-local release liveness in malformed MIR

- **Problem:** Array MIR verification validates the type and shape of emitted
  `Release` operations but does not maintain a complete live/released state for
  ordinary owning array locals. Removing or duplicating the final release of a
  valid array local can therefore survive verification even though lowering
  always emits the correct cleanup.
- **Evidence:** A hostile mutation of the shared-owner element-list fixture
  removed or duplicated `Release` for the completed inline outer array without
  producing a verifier error. Native lifecycle coverage still proved balanced
  generated cleanup for the unmodified program.
- **Likely owner:** Array ownership dataflow and cleanup verification, shared
  with all inline array categories rather than element-list lowering.
- **Priority:** Medium. This is a malformed-MIR hardening gap, not a source-
  reachable cleanup bug, and complete array-local liveness spans parameters,
  results, temporaries, assignments, fields, and conditional paths.
- **Useful implementation boundary:** Track every owning array place from
  initialization/adoption through replacement, transfer, release, and
  storage-dead; reject missing, duplicate, early, and type-mismatched cleanup
  at joins. Cover each storage role and both ordinary and element-list
  construction without conflating shared outer handles with inline backing.
