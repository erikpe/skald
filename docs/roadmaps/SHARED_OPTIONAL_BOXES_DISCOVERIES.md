# Shared Optional Boxes Discoveries

Status: pending.

This record keeps follow-up work found while implementing the completed shared
optional boxes roadmap separate from that milestone.

## Stabilize values across multiple control-flow-producing subexpressions

**Priority:** Medium.

**Status:** Pending.

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
