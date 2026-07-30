# Short-Circuit Boolean Expressions Discoveries

Status: pending follow-up after the completed short-circuit implementation.

## Compact equivalent path-state alternatives

- **Problem:** Path-sensitive verifier state currently stores one predicate map
  per selected logical-path alternative. Mixed logical chains can create many
  equivalent resource states distinguished only by predicates that remain live
  to the shared full-expression boundary.
- **Evidence:** Hardening stress tests showed that verifier time grows much
  faster than the linear logical CFG. The parser therefore caps one
  expression-tree path at 10 nested short-circuit operations so every accepted
  expression remains within a practical verification bound.
- **Likely owner:** Shared MIR path-state verification.
- **Priority:** Medium; required before raising or removing the logical-depth
  implementation limit.
- **Useful boundary:** Preserve exact parent selection, conditional cleanup,
  loop epochs, and existing verifier diagnostics while representing
  alternatives with identical resource state compactly. Benchmark mixed flat
  and right-nested chains and change the public limit only in a separately
  reviewed language/compiler update.

## Split optional initialization verification by responsibility

- **Problem:** `mir/verify/optional/initialization.rs` now owns path-sensitive
  fixed-point propagation, instruction and terminator diagnostics,
  exact-class optional ownership transfer, storage-epoch checks, and recursive
  optional-field seeding in one large module.
- **Evidence:** Path-dependent optional work required the same initialization
  state to flow through declared alternatives and to be consumed by
  class-optional and optional-shared cleanup, moved sources, and value-argument
  transfer. The resulting owner is cohesive but exceeds 900 lines. Bounded
  optional guards remained an immediate-consumer responsibility and did not
  justify expanding this module further.
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
- **Evidence:** Short-circuit ownership work made the existing state
  path-sensitive without weakening ordinary joins. The module now exceeds
  1,000 lines, and its instruction transition logic is independent from CFG
  alternative selection and condition convergence.
- **Likely owner:** MIR shared-ownership verification.
- **Priority:** Medium, after the short-circuit representation and verifier
  contracts stabilize.
- **Useful boundary:** Keep the existing `shared` verifier facade and exact
  diagnostics. Separate path-sensitive propagation from owner/allocation
  transitions and shared-place/checked-view use checks; keep `SharedState`
  private to the resulting sibling modules.
