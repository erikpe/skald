# Shared-Ownership Maintainability Discoveries

Status: pending follow-up after the shared-ownership implementation roadmap.

The shared-ownership closeout removed the obsolete HIR lowering gate and
hardened the external-signature trust boundary. The remaining items below are
maintainability work rather than missing language behavior.

## Split shared MIR verification by responsibility

**Priority:** medium.

`crates/skald-compiler/src/mir/verify/shared.rs` owns both instruction-local
structural checks and the path-sensitive allocation/owner/anchor state
analysis. At roughly 1,280 lines, those two independently understandable
responsibilities now obscure one another even though each is internally
cohesive.

Move the current structural `Verifier` methods behind a private
`verify/shared/structural.rs` module and the ownership dataflow state machine
behind `verify/shared/ownership.rs`, with a small `verify/shared/mod.rs`
facade. Preserve diagnostics, traversal order, and the existing
`verify_shared_ownership` entry point exactly. Do this as a mechanical
maintainability change with verifier and determinism tests, not alongside a
new ownership feature.

## Partition shared-ownership phase tests

**Priority:** low.

The type-check and MIR shared-ownership test files have grown into broad
milestone accumulations. Their coverage is valuable, but locating the owner of
a regression requires scanning unrelated call, field, cast, anchor, and copy
allocation cases.

Split each test module by semantic responsibility—core owners, calls/results,
fields, casts/views, anchors, and copy allocation—while retaining private
helpers only in the narrowest common parent. Keep native behavior in the
backend shared-ownership suite and complete source behavior in golden tests.
No production API or diagnostic change should accompany this reorganization.
