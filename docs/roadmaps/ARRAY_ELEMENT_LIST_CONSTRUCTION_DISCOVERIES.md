# Array Element-List Construction Discoveries

Status: actionable follow-up after the completed array element-list
construction roadmap.

## Optional shared unwrap directly initializing a local

- **Problem:** A checked optional shared-owner unwrap used directly as the
  initializer of an ordinary shared local lowers the unwrap result into that
  local, while structural MIR verification currently requires the result
  storage to be a temporary or shared anchor. The equivalent unwrap in a later
  expression works because it naturally receives temporary storage.
- **Evidence:** `var owner: shared Item = maybe_owner!;` reached the invalid-MIR
  assertion `optional shared unwrap requires matching optional source and fresh
  shared owner` in native array element-list coverage after first copying a
  present optional owner from an array slot. Presence tests and expression-
  temporary unwraps remain valid, so the element-list transfer itself is not
  implicated.
- **Likely owner:** Optional shared unwrap lowering and shared local
  initialization, with the structural and ownership verifiers preserving the
  established secure-owner protocol.
- **Priority:** Medium. Valid source can reach an internal invalid-MIR
  assertion, but repairing direct-local placement is a pre-existing optional
  lowering concern rather than part of array slot construction.
- **Useful implementation boundary:** Lower checked unwrap into a fresh typed
  shared temporary, then consume/adopt it into the destination local using the
  ordinary shared transfer path. Cover direct locals, fields, arguments,
  results, optional array-element sources, checked failure, exact target
  compatibility, and mutated MIR.

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
