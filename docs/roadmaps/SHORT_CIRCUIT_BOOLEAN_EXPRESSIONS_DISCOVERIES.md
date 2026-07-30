# Short-Circuit Boolean Expressions Discoveries

Status: pending; address after the short-circuit boolean expressions roadmap.

## Split optional initialization verification by responsibility

- **Problem:** `mir/verify/optional/initialization.rs` now owns path-sensitive
  fixed-point propagation, instruction and terminator diagnostics,
  exact-class optional ownership transfer, storage-epoch checks, and recursive
  optional-field seeding in one large module.
- **Evidence:** SC3 and SC4 required the same optional initialization state to
  flow through declared path alternatives and to be consumed by class-optional
  and optional-shared cleanup, moved sources, and value-argument transfer. The
  resulting owner is cohesive but exceeds 900 lines, and SC5 may add more
  path-sensitive optional interactions.
- **Likely owner:** MIR optional verification.
- **Priority:** Medium, after the short-circuit representation and verifier
  contracts stabilize.
- **Useful boundary:** Keep the existing `optional` verifier facade and exact
  diagnostics. Separate path-state propagation and condition convergence from
  instruction-local checks and recursive class-field initialization helpers;
  keep the definite-initialization state private to those modules.

## Split shared ownership verification by state responsibility

- **Problem:** `mir/verify/shared/ownership.rs` owns allocation publication,
  ordinary and static owners, transfers, call handoffs, field initialization,
  shared provenance, checked-view lifetime, full-expression state, and
  path-sensitive fixed-point propagation in one module.
- **Evidence:** SC4 made the existing ownership state path-sensitive without
  weakening ordinary joins. The module now exceeds 1,000 lines, and its
  instruction transition logic is independent from CFG alternative selection
  and condition convergence.
- **Likely owner:** MIR shared-ownership verification.
- **Priority:** Medium, after the short-circuit representation and verifier
  contracts stabilize.
- **Useful boundary:** Keep the existing `shared` verifier facade and exact
  diagnostics. Separate path-sensitive propagation from owner/allocation
  transitions and shared-place/checked-view use checks; keep `SharedState`
  private to the resulting sibling modules.
