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
