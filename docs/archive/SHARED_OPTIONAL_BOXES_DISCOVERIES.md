# Resolved Shared Optional Boxes Discoveries

Status: resolved.

This record preserves follow-up work found while implementing the completed
shared optional boxes roadmap.

## Stabilize values across multiple control-flow-producing subexpressions

**Priority:** Medium.

**Status:** Resolved.

**Problem:** A single arithmetic return expression that combined a checked
optional-box-owner unwrap, boxed optional unwrap, and a later internal call
could lower to preliminary MIR where a value was used in a block before its
definition. Binding the unwrap result and call result to separate locals before
the arithmetic expression produced valid MIR and identical intended behavior.

**Evidence:** The shared optional-box native array fixture initially combined
`(*optional_owner!)!` and an interface-dispatching array call in one addition
chain. Preliminary MIR verification rejected the resulting function with a
value-use-order error. Splitting both control-flow-producing operands into
preceding local initializers made the complete verified and native test pass.
The array and optional-box ownership protocols themselves remained valid.

**Likely owner:** `crates/skald-compiler/src/mir/lower/expression.rs`, the
optional unwrap lowering modules, and full-expression/control-flow value
stabilization.

**Useful boundary:** Add a minimal non-array reproducer, identify whether
binary-expression lowering retains a value across a block split without a
stable storage home, and introduce one general stabilization rule. Preserve
left-to-right evaluation, full-expression cleanup, guard/anchor lifetime, and
existing MIR value dominance rules; do not special-case arrays or optional
boxes.

**Resolution:** MIR lowering already had the general rule: an earlier scalar
is spilled before a later subexpression whose complete lowering can change
blocks. The control-effect summary incorrectly classified named by-value array
arguments by inspecting only their source receiver, overlooking the checked
allocation and deep-copy loop performed while preparing the argument. It also
treated an owning object copy from an array element as block-free despite its
checked position lowering. The summary now classifies both complete argument
preparation operations, so the existing general spill rule preserves values,
evaluation order, and full-expression lifetimes without an array or
optional-box branch in expression lowering. The original native box-array
fixture again uses one combined expression, while a separate non-array MIR
regression composes checked optional unwraps across an internal call.
