# Short-Circuit Boolean Expressions Discoveries

Status: pending; address after the short-circuit boolean expressions roadmap.

## Split optional initialization verification by responsibility

- **Problem:** `mir/verify/optional/initialization.rs` now owns path-sensitive
  fixed-point propagation, instruction and terminator diagnostics,
  exact-class optional ownership transfer, storage-epoch checks, and recursive
  optional-field seeding in one large module.
- **Evidence:** SC3 required the same optional initialization state to flow
  through declared path alternatives and to be consumed by class-optional
  cleanup and value-argument transfer. The resulting owner is cohesive but
  exceeds 800 lines, and SC4–SC5 may add more path-sensitive optional/shared
  interactions.
- **Likely owner:** MIR optional verification.
- **Priority:** Medium, after the short-circuit representation and verifier
  contracts stabilize.
- **Useful boundary:** Keep the existing `optional` verifier facade and exact
  diagnostics. Separate path-state propagation and condition convergence from
  instruction-local checks and recursive class-field initialization helpers;
  keep the definite-initialization state private to those modules.

