# Array Element-List Construction Discoveries

Status: actionable follow-up after the active array element-list construction
roadmap.

## Scalar call arguments preceding control-flow-producing arguments

- **Problem:** Call and initializer lowering can leave an early primitive
  argument as a block-local MIR value when a later argument introduces control
  flow. A nested array element-list allocation in that later argument then
  moves the call into a successor block, where MIR verification correctly
  rejects the earlier value as not defined in the current block.
- **Evidence:** A recursive `Node` initializer shaped as
  `Node(1, Node[]{...})` produced `value ... is used before it is defined in
  this block` during verified HIR-to-MIR lowering. Reordering the equivalent
  signature to place the array argument first avoids the invalid cross-block
  value use.
- **Likely owner:** MIR call and object-initializer argument lowering, together
  with the existing scalar-spill and full-expression storage machinery.
- **Priority:** Medium. Valid argument expressions can currently reach an
  internal invalid-MIR assertion, but the issue is not specific to array
  element-list ownership and requires a cross-cutting call-lowering change.
- **Useful implementation boundary:** Before lowering a later argument that
  may split control flow, materialize already evaluated block-local arguments
  into typed spill storage, preserve source-order effects and ownership, then
  reload them in the final call block. Add direct-call, method, initializer,
  and nested conditional/allocation argument tests plus malformed-MIR and MSRV
  validation.

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
